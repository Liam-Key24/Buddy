//! Native tool-call agent loop: Brain `/v1/complete` + Core execution.

use std::collections::HashSet;
use std::time::Instant;

use buddy_calendar::parse_when_label;
use buddy_memory::{HistoryMessage, MemoryContext};
use buddy_personality::{phrase_tool_result, style_response, PersonalityProfile};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tracing::{info, warn};

use crate::calendar_look;
use crate::inference_gateway::{self, CompleteHttp, CompleteKind};
use crate::memory_extraction::BrainMemoryContext;
use crate::orchestrator::{
    emit_trace, execute_tool_step, AgentTurn, ScratchStep, ToolStepOutcome,
};
use crate::run_control::RunGuard;
use crate::runtime_policy::{tool_fingerprint, RuntimePolicy};
use crate::state::AppState;
use crate::turn_trace::{TurnPath, TurnTrace};

/// Internal complete-loop flag, mapped from [`crate::turn_controller::ModelLane`].
/// Not a product-facing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatMode {
    Talk,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NativeTranscript {
    pub goal: String,
    #[serde(default)]
    pub messages: Vec<Value>,
    #[serde(default)]
    pub scratchpad: Vec<ScratchStep>,
    #[serde(default)]
    pub last_organize_input: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct WorkspaceFocus {
    pub pages: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_doc: Option<String>,
}


pub enum NativeOutcome {
    Done(String),
    Paused(String),
    Stopped(String),
    Fallback(String),
}

pub fn load_transcript(state: &AppState, conversation_id: &str) -> Option<NativeTranscript> {
    // Compatibility reader: WorkItem is authoritative; this still understands
    // a nested NativeTranscript without thinning it to a scratchpad.
    let raw = state.memory.get_agent_turn(conversation_id)?;
    serde_json::from_str::<NativeTranscript>(&raw)
        .ok()
        .filter(|t| !t.messages.is_empty())
}

pub fn save_transcript(state: &AppState, conversation_id: &str, transcript: &NativeTranscript) {
    if let Ok(raw) = serde_json::to_string(transcript) {
        state.memory.set_agent_turn(conversation_id, &raw);
    }
}

fn memory_block(memory: &BrainMemoryContext) -> String {
    let mut parts = Vec::new();
    for (label, value) in [
        ("Handover", memory.handover.as_deref()),
        ("Working", memory.working.as_deref()),
        ("Preferences", memory.preferences.as_deref()),
        ("Decisions", memory.decisions.as_deref()),
        ("Active sparks", memory.active_sparks.as_deref()),
        ("Todos", memory.open_todos.as_deref()),
        ("Fitness", memory.fitness.as_deref()),
        ("Study", memory.study.as_deref()),
        ("Money", memory.money.as_deref()),
        ("Socials", memory.socials.as_deref()),
    ] {
        if let Some(v) = value.filter(|s| !s.trim().is_empty()) {
            parts.push(format!("{label}: {v}"));
        }
    }
    parts.join("\n")
}

fn talk_system_prompt(memory: &BrainMemoryContext) -> String {
    let now = chrono::Local::now();
    let now_iso = now.to_rfc3339();
    let now_ms = now.timestamp_millis();
    let mem = memory_block(memory);
    format!(
        "You are Buddy, a helpful local AI assistant. This turn is conversation only — \
you cannot call tools or change calendar, documents, todos, fitness, or money. \
Answer clearly and directly in plain text. No <think> tags, no JSON, no tool calls.\n\
Current local time: {now_iso} ({now_ms} ms).\n\
{mem}"
    )
}

fn agent_system_prompt(memory: &BrainMemoryContext) -> String {
    let now = chrono::Local::now();
    let now_iso = now.to_rfc3339();
    let now_ms = now.timestamp_millis();
    let mem = memory_block(memory);
    format!(
        "You are Buddy, a local AI assistant. Talk naturally. Call a tool only when this \
message needs to read or change the user's data; otherwise reply in plain text.\n\
A day dump may need several tools in one turn (food + money + spark + doc + calendar). Call all that apply.\n\
Current local time: {now_iso} ({now_ms} ms).\n\
Calendar: pin only with an explicit clock time; otherwise organize. Always propose first (mode=propose) unless the user confirmed.\n\
calendar.look when=today|tomorrow|this_week|next_week|weekend. \"next week plans?\" is look, not organize.\n\
Life tools: todo.list/add/update, docs.search/get/list/format/patch/upsert/delete, study.status/look/log_session/upsert_subject/upsert_topic/upsert_assignment, fitness.summary/look/log_food/log_workout/log_weight/fridge/climbing_stats, money.summary/list/analyze/log/pots/pot, socials.get_plan/look/summary, research.list/get/update, list_sparks/save_spark/update_spark.\n\
One user message may need several tools (spark + todos + a free slot tomorrow). Call all that apply, in order: look/read first, then write, then calendar.organize mode=propose for flexible time. Easy → priority low. Tomorrow → deadline tomorrow and/or window=tomorrow. Invented essays → docs.upsert with a real body. Study event → calendar.look then study.look then upsert topic/assignments (create subject if missing). Pasted syllabus with Module N / Assignment: Module Quiz / Final Exam → one course topic (e.g. Intro to Cybersecurity); each Module Quiz → study.upsert_assignment kind=assignment titled \"Module N Quiz\"; Final Exam → kind=exam. Never invent fragment titles like Api/Cloud from bullet jargon. Always pass subject or subject_id. Never pin without an explicit clock time.\n\
Read before answering about their data. Digests are one-liners — call look/list/get for the actual rows (what they ate, lifted, spent, studied, researched, or sparked).\n\
Food: if they ate something without macros, estimate a typical serving, log via fitness.log_food, and say the numbers are estimates.\n\
Workouts: log via fitness.log_workout with named sets (exercise/reps/weight). Weight: fitness.log_weight kg=.\n\
Money: spent/earned → money.log (pounds like 12.50). Not work sales. What they spent → money.list. Savings split → money.pot (named pots) / money.pots. “holiday pot £200, emergency £500” sets balances; “put £50 in holiday” adds.\n\
Documents: better/cleaner format → docs.format (tiny call, never rewrite the body). Small edit → docs.patch. Create or paste a new body → docs.upsert. If <ui_context> names an open doc, use that id. write_file is home-folder disk only.\n\
Socials never invent progress or GitHub posts; approved posts go to Calendar only after the user confirms.\n\
Return tool calls via the tools API (or <tool_call>{{\"name\":\"...\",\"arguments\":{{}}}}</tool_call>).\n\
Do not invent unix timestamps when a relative phrase like \"tomorrow 14:00\" or window=this_week works.\n\
{mem}"
    )
}

fn history_to_messages(history: &[HistoryMessage]) -> Vec<Value> {
    history
        .iter()
        .rev()
        .take(16)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .filter(|m| !m.content.trim().is_empty())
        .map(|m| {
            json!({
                "role": m.role,
                "content": m.content,
            })
        })
        .collect()
}

fn arguments_string(args: &Value) -> String {
    match args {
        Value::String(s) => {
            if s.trim().is_empty() {
                "{}".into()
            } else {
                s.clone()
            }
        }
        Value::Null => "{}".into(),
        other => other.to_string(),
    }
}

async fn brain_complete(
    app: &AppHandle,
    state: &AppState,
    messages: &[Value],
    tools: &[Value],
    run: &RunGuard,
    policy: &RuntimePolicy,
    kind: CompleteKind,
    conversation_id: &str,
    turn_id: &str,
) -> Result<CompleteHttp, String> {
    inference_gateway::complete_live(
        app,
        state,
        run,
        policy,
        messages,
        tools,
        kind,
        conversation_id,
        turn_id,
    )
    .await
    .map_err(|e| e.as_str().to_string())
}

/// Run or resume the native tool loop.
#[allow(clippy::too_many_arguments)]
pub async fn run_native_turn(
    app: &AppHandle,
    state: &AppState,
    _client: &reqwest::Client,
    ctx: &MemoryContext,
    conversation_id: &str,
    text: &str,
    ui_context: Option<&str>,
    history: &[HistoryMessage],
    memory: &BrainMemoryContext,
    personality: &PersonalityProfile,
    resume: bool,
    run: &RunGuard,
    chat_mode: ChatMode,
    trace: &mut TurnTrace,
    policy: &RuntimePolicy,
    _started: Instant,
) -> Result<NativeOutcome, String> {
    let conv_kind = state
        .db
        .get_conversation(conversation_id)
        .ok()
        .map(|c| c.kind)
        .unwrap_or_default();
    let tools = tools_for_turn(state, text, ui_context, &conv_kind, chat_mode);
    trace.attached_tool_count = tools.len() as u32;
    trace.model = Some("qwen".into());
    let mut transcript = if resume {
        load_transcript(state, conversation_id).unwrap_or_else(|| NativeTranscript {
            goal: text.to_string(),
            messages: Vec::new(),
            scratchpad: Vec::new(),
            last_organize_input: None,
        })
    } else {
        NativeTranscript {
            goal: text.to_string(),
            messages: Vec::new(),
            scratchpad: Vec::new(),
            last_organize_input: None,
        }
    };

    if transcript.messages.is_empty() {
        let mut sys = if chat_mode == ChatMode::Talk {
            talk_system_prompt(memory)
        } else {
            agent_system_prompt(memory)
        };
        if conv_kind == "research" && chat_mode != ChatMode::Talk {
            sys.push_str(
                "\nThis is a Deep Research conversation using the local 14B model only. \
After investigating, call research.update with structured fields: question, summary, findings[], sources[], details, open_questions, next_steps. \
Keep the chat reply useful; put structure in the tool.",
            );
        }
        let lower = text.trim().to_ascii_lowercase();
        if mixed_life_intent(&lower)
            && (lower.contains("social")
                || lower.contains("linkedin")
                || lower.contains("twitter")
                || lower.contains("weekly review")
                || lower.contains("posting"))
        {
            sys.push_str(
                "\nSocials: ground posts in this week's real notes only. Continuous story, not disconnected tips. \
No generic listicles. Honest/curious/technical tone. Media = real screenshots or photos to gather — never invent images. \
Skip GitHub. Nothing goes to Calendar until the user approves, then socials.commit_approved.",
            );
        }
        let mut msgs = vec![json!({
            "role": "system",
            "content": sys,
        })];
        msgs.extend(history_to_messages(history));
        msgs.push(json!({ "role": "user", "content": model_user_text(text, ui_context) }));
        transcript.messages = msgs;
    } else {
        transcript
            .messages
            .push(json!({ "role": "user", "content": model_user_text(text, ui_context) }));
    }

    emit_trace(
        app,
        "planning",
        if chat_mode == ChatMode::Talk {
            "Writing reply"
        } else {
            "Native tool loop"
        },
    );
    let mut last_content = String::new();
    let mut used_tools: Vec<String> = Vec::new();
    let mut phrased: Vec<String> = Vec::new();
    let mut last_doc_output: Option<String> = None;
    let mut seen_tools: HashSet<String> = HashSet::new();
    let max_steps = policy.max_model_calls.max(1) as usize;
    let planned_tools = 0u32;
    let turn_id = state
        .memory
        .get_work_item(conversation_id)
        .and_then(|w| w.turn_id)
        .unwrap_or_else(|| format!("turn:{conversation_id}"));
    let mut ready_at: Option<Instant> = None;

    for step in 0..max_steps {
        if run.is_cancelled() {
            save_transcript(state, conversation_id, &transcript);
            return Ok(stopped_outcome(app, state, conversation_id, personality));
        }
        if let Some(ready) = ready_at {
            if ready.elapsed() >= policy.turn_deadline() {
                trace.safety_budget = true;
                trace.exit_reason = Some("deadline".into());
                trace.set_path(TurnPath::Deadline);
                save_transcript(state, conversation_id, &transcript);
                let content = budget_message(personality, used_tools.len() as u32, planned_tools, "the time limit");
                let _ = app.emit("chat-chunk", &content);
                return Ok(NativeOutcome::Paused(content));
            }
        }
        if trace.model_call_count >= policy.max_model_calls {
            break;
        }
        emit_trace(
            app,
            "planning",
            &format!("Waiting on model ({}/{})", step + 1, max_steps),
        );
        let kind = if chat_mode == ChatMode::Talk {
            CompleteKind::Chat
        } else {
            CompleteKind::Interpret
        };
        let mut complete = match brain_complete(
            app,
            state,
            &transcript.messages,
            &tools,
            run,
            policy,
            kind,
            conversation_id,
            &turn_id,
        )
        .await
        {
            Ok(c) => {
                if ready_at.is_none() {
                    ready_at = Some(Instant::now());
                }
                trace.model_call_count += 1;
                c
            }
            Err(err) => {
                warn!(error = %err, step, "native complete failed");
                if run.is_cancelled() || err.contains("Stopped") {
                    save_transcript(state, conversation_id, &transcript);
                    return Ok(stopped_outcome(app, state, conversation_id, personality));
                }
                if err.contains("took too long") || err.contains("safe working time") {
                    save_transcript(state, conversation_id, &transcript);
                    let content = style_response(personality, &err);
                    let _ = app.emit("chat-chunk", &content);
                    return Ok(NativeOutcome::Paused(content));
                }
                if step == 0 && !resume {
                    return Ok(NativeOutcome::Fallback(err));
                }
                save_transcript(state, conversation_id, &transcript);
                let content = style_response(
                    personality,
                    "The local model is unavailable. Completed work is saved.",
                );
                let _ = app.emit("chat-chunk", &content);
                return Ok(NativeOutcome::Paused(content));
            }
        };

        if chat_mode == ChatMode::Talk && !complete.tool_calls.is_empty() {
            complete.tool_calls.clear();
            if complete
                .content
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                complete.content = Some("I can only chat right now.".into());
            }
        }

        if !complete.tool_calls.is_empty() {
            let planned = complete.tool_calls.len() as u32;
            let openai_calls: Vec<Value> = complete
                .tool_calls
                .iter()
                .map(|c| {
                    json!({
                        "id": c.id,
                        "type": "function",
                        "function": {
                            "name": c.name,
                            "arguments": arguments_string(&c.arguments),
                        }
                    })
                })
                .collect();
            transcript.messages.push(json!({
                "role": "assistant",
                "content": complete.content.clone().unwrap_or_default(),
                "tool_calls": openai_calls,
            }));

            let mut capped = false;
            for call in &complete.tool_calls {
                if ready_at.is_some_and(|t| t.elapsed() >= policy.turn_deadline()) {
                    trace.safety_budget = true;
                    trace.exit_reason = Some("deadline".into());
                    trace.set_path(TurnPath::Deadline);
                    capped = true;
                    break;
                }
                if trace.tool_steps >= policy.max_tool_executions {
                    trace.safety_budget = true;
                    trace.exit_reason = Some("tool_budget".into());
                    trace.set_path(TurnPath::Budget);
                    capped = true;
                    break;
                }
                let tool_name = remap_legacy_tool(&call.name);
                let mut input = arguments_string(&call.arguments);
                if tool_name != call.name {
                    input = remap_legacy_input(&call.name, &input);
                }
                let fingerprint = tool_fingerprint(&tool_name, &input);
                if !seen_tools.insert(fingerprint) {
                    trace.safety_budget = true;
                    trace.exit_reason = Some("repeat_tool".into());
                    trace.set_path(TurnPath::Budget);
                    capped = true;
                    break;
                }
                let dummy_turn = AgentTurn {
                    goal: transcript.goal.clone(),
                    scratchpad: transcript.scratchpad.clone(),
                };
                let idem = crate::runtime_policy::action_idempotency_key(&turn_id, trace.tool_steps);
                input = inject_idempotency_key(&tool_name, &input, &idem);
                if state
                    .memory
                    .get_work_item(conversation_id)
                    .is_some_and(|w| w.already_did(&idem))
                {
                    info!(tool = %tool_name, %idem, "skipping duplicate mutation");
                    transcript.messages.push(json!({
                        "role": "tool",
                        "tool_call_id": call.id,
                        "name": tool_name,
                        "content": "{\"status\":\"idempotent_reuse\"}",
                    }));
                    continue;
                }
                let output = match execute_tool_step(
                    app,
                    state,
                    ctx,
                    conversation_id,
                    text,
                    personality,
                    &tool_name,
                    &input,
                    &dummy_turn,
                )
                .await?
                {
                    ToolStepOutcome::NeedsUser(content) => {
                        used_tools.push(tool_name.clone());
                        trace.tool_steps += 1;
                        trace.clarification_count += 1;
                        trace.approval_stopped = true;
                        transcript.messages.push(json!({
                            "role": "tool",
                            "tool_call_id": call.id,
                            "name": tool_name,
                            "content": "{\"status\":\"needs_user\"}",
                        }));
                        emit_workspace_focus(app, &used_tools, last_doc_output.as_deref());
                        save_transcript(state, conversation_id, &transcript);
                        return Ok(NativeOutcome::Paused(content));
                    }
                    ToolStepOutcome::Done { output, .. } => {
                        if let Some(mut item) = state.memory.get_work_item(conversation_id) {
                            item.remember_idempotency(idem.clone());
                            state.memory.set_work_item(item);
                        }
                        output
                    }
                    ToolStepOutcome::Failed { output, .. } => output,
                };

                transcript.messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call.id,
                    "name": tool_name,
                    "content": output,
                }));
                transcript.scratchpad.push(ScratchStep {
                    tool: tool_name.clone(),
                    summary: compact(&tool_name, &output),
                });

                if tool_name == "calendar.organize" {
                    transcript.last_organize_input = Some(input.clone());
                }
                phrased.push(phrase_tool_result(&tool_name, &output));
                used_tools.push(tool_name.clone());
                trace.tool_steps += 1;
                if tool_name.starts_with("docs.") {
                    last_doc_output = Some(output.clone());
                }
                if tool_name == "calendar.look" {
                    calendar_look::remember_look(state, conversation_id, &output);
                }
                remember_life_look(state, conversation_id, &tool_name, &input);
            }
            emit_workspace_focus(app, &used_tools, last_doc_output.as_deref());
            save_transcript(state, conversation_id, &transcript);
            if should_follow_up_complete(
                &used_tools,
                trace.model_call_count,
                policy.max_model_calls,
                capped,
            ) && step + 1 < max_steps
            {
                continue;
            }
            let mut content = if phrased.is_empty() {
                style_response(personality, "I couldn't complete those actions.")
            } else {
                style_response(personality, &phrased.join("\n"))
            };
            if capped {
                content = budget_message(personality, used_tools.len() as u32, planned, "the safe work limit");
            } else {
                state.memory.clear_pending_clarification(conversation_id);
            }
            let _ = app.emit("chat-chunk", &content);
            return Ok(if capped {
                NativeOutcome::Paused(content)
            } else {
                NativeOutcome::Done(content)
            });
        }

        let text_out = complete.content.unwrap_or_default();
        if text_out.trim().is_empty() && last_content.is_empty() {
            last_content = style_response(personality, "I'm here to help.");
        } else if !text_out.trim().is_empty() {
            last_content = style_response(personality, &text_out);
        }
        emit_workspace_focus(app, &used_tools, last_doc_output.as_deref());
        let _ = app.emit("chat-chunk", &last_content);
        state.memory.clear_pending_clarification(conversation_id);
        return Ok(NativeOutcome::Done(last_content));
    }

    save_transcript(state, conversation_id, &transcript);
    trace.safety_budget = true;
    trace.exit_reason = Some("model_budget".into());
    trace.set_path(TurnPath::Budget);
    let content = budget_message(personality, used_tools.len() as u32, planned_tools, "the safe work limit");
    emit_workspace_focus(app, &used_tools, last_doc_output.as_deref());
    let _ = app.emit("chat-chunk", &content);
    Ok(NativeOutcome::Paused(content))
}

fn budget_message(
    personality: &PersonalityProfile,
    completed: u32,
    planned: u32,
    limit_name: &str,
) -> String {
    let remain = planned.saturating_sub(completed);
    let body = if planned > 0 && remain > 0 {
        format!(
            "I stopped this turn after completing {completed} of {planned} actions because it reached {limit_name}. The completed actions are saved. {remain} actions remain—continue when you’re ready."
        )
    } else if completed > 0 {
        format!(
            "I stopped this turn after completing {completed} actions because it reached {limit_name}. The completed actions are saved. Continue when you’re ready."
        )
    } else {
        format!("I stopped this turn because it reached {limit_name}. Nothing new was saved.")
    };
    style_response(personality, &body)
}

pub fn should_follow_up_complete(
    used_tools: &[String],
    model_calls: u32,
    max_model_calls: u32,
    capped: bool,
) -> bool {
    !capped
        && !used_tools.is_empty()
        && used_tools
            .iter()
            .all(|n| inference_gateway::is_readonly_tool(n))
        && model_calls < max_model_calls
}

fn inject_idempotency_key(tool_name: &str, input: &str, key: &str) -> String {
    if tool_name != "goal.intake" {
        return input.to_string();
    }
    match serde_json::from_str::<Value>(input) {
        Ok(Value::Object(mut map)) => {
            map.insert("idempotency_key".into(), Value::String(key.to_string()));
            Value::Object(map).to_string()
        }
        _ => input.to_string(),
    }
}

fn stopped_outcome(
    app: &AppHandle,
    _state: &AppState,
    _conversation_id: &str,
    personality: &PersonalityProfile,
) -> NativeOutcome {
    let content = style_response(
        personality,
        inference_gateway::stopped_copy(true),
    );
    let _ = app.emit("chat-chunk", &content);
    NativeOutcome::Stopped(content)
}

fn compact(tool: &str, output: &str) -> String {
    let trimmed = output.trim();
    if trimmed.len() <= 800 {
        format!("{tool}: {trimmed}")
    } else {
        format!("{tool}: {}…", &trimmed[..800])
    }
}

fn remap_legacy_tool(name: &str) -> String {
    match name {
        "calendar.get_today" | "calendar.get_tomorrow" | "calendar.get_this_week"
        | "calendar.find_free_time" | "calendar.search_events" | "calendar.list_events" => {
            "calendar.look".into()
        }
        "calendar.create_event" | "calendar.update_event" | "calendar.delete_event" => {
            "calendar.pin".into()
        }
        "calendar.schedule_task" | "calendar.plan_day" | "calendar.block_time" => {
            "calendar.organize".into()
        }
        other => other.to_string(),
    }
}

fn remap_legacy_input(original: &str, input: &str) -> String {
    let mut v: Value = serde_json::from_str(input).unwrap_or_else(|_| json!({}));
    if !v.is_object() {
        v = json!({});
    }
    let obj = v.as_object_mut().unwrap();
    match original {
        "calendar.get_today" => {
            obj.insert("when".into(), json!("today"));
        }
        "calendar.get_tomorrow" => {
            obj.insert("when".into(), json!("tomorrow"));
        }
        "calendar.get_this_week" => {
            obj.insert("when".into(), json!("this_week"));
        }
        "calendar.find_free_time" => {
            obj.entry("focus").or_insert(json!("free"));
            if !obj.contains_key("when") {
                obj.insert("when".into(), json!("this_week"));
            }
        }
        "calendar.create_event" => {
            obj.entry("action").or_insert(json!("create"));
            if let Some(st) = obj.remove("start_time") {
                obj.insert("start".into(), st);
            }
            if let Some(et) = obj.remove("end_time") {
                obj.insert("end".into(), et);
            }
        }
        "calendar.update_event" => {
            obj.insert("action".into(), json!("update"));
        }
        "calendar.delete_event" => {
            obj.insert("action".into(), json!("delete"));
        }
        "calendar.schedule_task" | "calendar.plan_day" | "calendar.block_time" => {
            obj.entry("mode").or_insert(json!("propose"));
            if !obj.contains_key("items") {
                if let Some(title) = obj.get("title").cloned() {
                    let mut item = json!({ "title": title });
                    if let Some(d) = obj.get("duration_minutes") {
                        item["duration_minutes"] = d.clone();
                    }
                    if let Some(c) = obj.get("count") {
                        item["count"] = c.clone();
                    }
                    obj.insert("items".into(), json!([item]));
                } else if let Some(tasks) = obj.get("tasks").cloned() {
                    obj.insert("items".into(), tasks);
                }
            }
            if obj.get("prefer_after_work").and_then(|v| v.as_bool()) == Some(true) {
                let mut c = obj
                    .get("constraints")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                if !c.iter().any(|x| x.as_str() == Some("after_work")) {
                    c.push(json!("after_work"));
                }
                obj.insert("constraints".into(), json!(c));
            }
            obj.entry("window").or_insert(json!("this_week"));
        }
        _ => {}
    }
    v.to_string()
}

fn is_life_readish(lower: &str) -> bool {
    has_any(
        lower,
        &[
            "list ",
            "show ",
            "what's",
            "whats ",
            "what did",
            "what do",
            "what have",
            "in my",
            "do i have",
            "have i",
            "whats in",
            "what's in",
            "how much",
            "open my",
            "open the",
            "read my",
            "read the",
        ],
    ) || lower.starts_with("list")
        || lower.starts_with("show")
        || lower.starts_with("open ")
}

fn is_life_writeish(lower: &str) -> bool {
    has_any(
        lower,
        &[
            "add ",
            "create ",
            "log a ",
            "log my ",
            "save ",
            "schedule ",
            "i spent",
            "i paid",
            "i earned",
            "i ate",
            "i had ",
            "i studied",
            "remind me to",
        ],
    ) && !is_life_readish(lower)
}

/// Obvious tracker reads — skip MLX (same idea as calendar.look).
pub fn fast_life_look_intent(text: &str) -> Option<(&'static str, String)> {
    let lower = text.trim().to_ascii_lowercase();
    if lower.is_empty() || looks_like_organize(&lower) || looks_like_clock_pin(&lower) {
        return None;
    }
    if is_life_writeish(&lower) {
        return None;
    }
    if lower.contains("should") || lower.contains("suggest") {
        return None;
    }
    if (has_word(&lower, "study") || lower.contains("studied"))
        && has_calendar_anchor(&lower)
    {
        return None;
    }
    let readish = is_life_readish(&lower) || looks_like_spark_list(&lower);
    if !readish {
        return None;
    }
    if looks_like_doc_look(&lower) {
        return Some(fast_docs_look_pair(&lower));
    }
    if has_word(&lower, "workout") || lower.contains("workouts") {
        return Some(("fitness.look", json!({ "what": "workouts" }).to_string()));
    }
    if lower.contains("fridge") {
        return Some(("fitness.look", json!({ "what": "fridge" }).to_string()));
    }
    if has_word(&lower, "weight") || has_word(&lower, "weigh") {
        return Some(("fitness.look", json!({ "what": "weight" }).to_string()));
    }
    if lower.contains("climb") || lower.contains("bouldering") {
        return Some(("fitness.look", json!({ "what": "climbs" }).to_string()));
    }
    if has_word(&lower, "eat")
        || has_word(&lower, "ate")
        || has_word(&lower, "food")
        || lower.contains("meals")
    {
        return Some(("fitness.look", json!({ "what": "food" }).to_string()));
    }
    if has_word(&lower, "study") || lower.contains("studied") {
        let what = if lower.contains("assignment") {
            "assignments"
        } else if lower.contains("topic") {
            "topics"
        } else {
            "sessions"
        };
        return Some(("study.look", json!({ "what": what }).to_string()));
    }
    if has_word(&lower, "pot") || has_word(&lower, "pots") || lower.contains("savings split") {
        return Some(("money.pots", "{}".into()));
    }
    if has_any(
        &lower,
        &["spent", "spend", "expense", "expenses", "transactions", "earned"],
    ) || has_word(&lower, "money")
    {
        return Some(("money.list", "{}".into()));
    }
    if looks_like_spark_list(&lower) {
        return Some(("list_sparks", json!({ "status": "active" }).to_string()));
    }
    if has_any(&lower, &["todo", "to-do", "to do", "my tasks"]) {
        return Some(("todo.list", "{}".into()));
    }
    if has_any(
        &lower,
        &["linkedin", "socials", "social draft", "social idea"],
    ) || (has_word(&lower, "drafts") && has_any(&lower, &["social", "post"]))
        || has_word(&lower, "ideas") && has_any(&lower, &["social", "post", "content"])
    {
        let what = if lower.contains("draft") {
            "drafts"
        } else if lower.contains("published") {
            "published"
        } else if lower.contains("thread") {
            "threads"
        } else {
            "ideas"
        };
        return Some(("socials.look", json!({ "what": what }).to_string()));
    }
    if has_word(&lower, "research") {
        return Some(("research.list", "{}".into()));
    }
    None
}

pub(crate) fn looks_like_doc_look(lower: &str) -> bool {
    let noun = has_word(lower, "sheet")
        || has_word(lower, "document")
        || has_word(lower, "documents")
        || has_word(lower, "docs")
        || has_word(lower, "notebook")
        || (has_word(lower, "notes") && !has_word(lower, "spark"));
    if !noun {
        return false;
    }
    is_life_readish(lower) || has_any(lower, &["open ", "read ", "show "])
}

fn fast_docs_look_pair(lower: &str) -> (&'static str, String) {
    let query = doc_look_query(lower);
    if query.is_empty()
        || matches!(
            query.as_str(),
            "documents" | "docs" | "notes" | "sheets" | "document"
        )
    {
        return ("docs.list", "{}".into());
    }
    (
        "docs.search",
        json!({ "query": query, "limit": 8 }).to_string(),
    )
}

fn doc_look_query(lower: &str) -> String {
    let mut t = lower
        .trim()
        .trim_end_matches(['?', '.', '!'])
        .trim()
        .to_string();
    const PREFIXES: &[&str] = &[
        "what's in my ",
        "whats in my ",
        "what is in my ",
        "what's in the ",
        "whats in the ",
        "what is in the ",
        "what's in ",
        "whats in ",
        "what is in ",
        "show me my ",
        "show my ",
        "show me the ",
        "list my ",
        "list the ",
        "open my ",
        "open the ",
        "read my ",
        "read the ",
        "what's on my ",
        "whats on my ",
    ];
    for p in PREFIXES {
        if let Some(rest) = t.strip_prefix(p) {
            t = rest.trim().to_string();
            break;
        }
    }
    t
}

pub fn is_life_followup(text: &str) -> bool {
    let lower = text.trim().to_ascii_lowercase();
    let t = lower.trim_end_matches(['?', '.', '!']).trim();
    t == "list them"
        || t == "show them"
        || t == "list those"
        || t == "show those"
        || t == "list it"
        || t == "show it"
        || t == "all of them"
        || t == "the list"
        || t.contains("list them")
        || t.contains("show them")
        || t.contains("list those")
        || t.contains("show those")
}

fn life_look_from_ui(ui_context: Option<&str>) -> Option<(&'static str, String)> {
    let page = ui_context.unwrap_or("").to_ascii_lowercase();
    if page.contains("page: fitness") {
        let what = if page.contains("section workout") {
            "workouts"
        } else if page.contains("section food") {
            "food"
        } else {
            "summary"
        };
        return Some(("fitness.look", json!({ "what": what }).to_string()));
    }
    if page.contains("page: study") {
        return Some(("study.look", json!({ "what": "sessions" }).to_string()));
    }
    if page.contains("page: money") {
        return Some(("money.list", "{}".into()));
    }
    if page.contains("page: todo") {
        return Some(("todo.list", "{}".into()));
    }
    if page.contains("page: documents") {
        return Some(("docs.list", "{}".into()));
    }
    if page.contains("page: socials") {
        return Some(("socials.look", json!({ "what": "ideas" }).to_string()));
    }
    if page.contains("page: research") {
        return Some(("research.list", "{}".into()));
    }
    None
}

fn saved_life_look(raw: &str) -> Option<(String, String)> {
    let v: Value = serde_json::from_str(raw).ok()?;
    let tool = v.get("tool")?.as_str()?.to_string();
    let input = v
        .get("input")
        .and_then(|i| i.as_str())
        .unwrap_or("{}")
        .to_string();
    if tool.is_empty() {
        return None;
    }
    Some((tool, input))
}

pub fn remember_life_look(state: &AppState, conversation_id: &str, tool: &str, input: &str) {
    if !matches!(
        tool,
        "fitness.look"
            | "fitness.summary"
            | "study.look"
            | "study.status"
            | "money.list"
            | "money.summary"
            | "money.pots"
            | "todo.list"
            | "list_sparks"
            | "socials.look"
            | "research.list"
            | "research.get"
            | "docs.search"
            | "docs.get"
            | "docs.list"
    ) {
        return;
    }
    let raw = json!({ "tool": tool, "input": input }).to_string();
    state.memory.set_last_life_look_raw(conversation_id, &raw);
}

pub fn resolve_life_look_intent(
    text: &str,
    ui_context: Option<&str>,
    history: &[HistoryMessage],
    saved_raw: Option<&str>,
) -> Option<(String, String)> {
    if let Some((tool, input)) = fast_life_look_intent(text) {
        return Some((tool.to_string(), input));
    }
    let lower = text.trim().to_ascii_lowercase();
    let follow = is_life_followup(text) || is_life_readish(&lower);
    if !follow {
        return None;
    }
    if let Some(raw) = saved_raw {
        if let Some(pair) = saved_life_look(raw) {
            return Some(pair);
        }
    }
    for msg in history.iter().rev().take(8) {
        if msg.role != "user" {
            continue;
        }
        if is_life_followup(&msg.content) {
            continue;
        }
        if let Some((tool, input)) = fast_life_look_intent(&msg.content) {
            return Some((tool.to_string(), input));
        }
    }
    if let Some((tool, input)) = life_look_from_ui(ui_context) {
        return Some((tool.to_string(), input));
    }
    None
}


#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LifeDump {
    pub money: bool,
    pub food: bool,
    pub curriculum: bool,
}

impl LifeDump {
    pub fn blocks_complete(self) -> bool {
        self.money || self.food || self.curriculum
    }
}

fn page_hint(ui_context: Option<&str>) -> String {
    ui_context.unwrap_or("").to_ascii_lowercase()
}

fn on_page(page: &str, name: &str) -> bool {
    page.contains(&format!("page: {name}"))
}

/// Classify obvious log dumps that must never wait on `/v1/complete`.
pub fn classify_life_dump(text: &str, ui_context: Option<&str>) -> LifeDump {
    let lower = text.trim().to_ascii_lowercase();
    if lower.is_empty() || looks_like_organize(&lower) {
        return LifeDump::default();
    }
    let question = lower.contains('?') || is_life_readish(&lower);
    let page = page_hint(ui_context);
    let mut dump = LifeDump {
        curriculum: parse_study_curriculum(text).is_some(),
        ..LifeDump::default()
    };
    let pot_pairs = parse_money_pots(&lower);
    if question && pot_pairs.is_none() {
        return dump;
    }
    let money_page = on_page(&page, "money");
    let food_page = on_page(&page, "fitness") || page.contains("calorie") || page.contains("food");
    let amount = parse_money_amount(&lower).is_some();
    let money_cue = has_any(
        &lower,
        &[
            "spent",
            "paid",
            "bought",
            "earned",
            "income",
            "expense",
            "rent",
            "bill",
            "mortgage",
            " was £",
            " is £",
            " pot",
            "pots",
        ],
    ) || (lower.contains(" was ") && lower.contains('£'));
    dump.money = pot_pairs.is_some() || (amount && (money_page || money_cue));

    let ate = has_any(&lower, &["i ate ", " ate "]);
    let had = has_any(&lower, &["i had ", "i've had ", "had a ", "had an "]);
    let food_cue = has_any(
        &lower,
        &[
            "calorie",
            "calories",
            "breakfast",
            "lunch",
            "dinner",
            "snack",
            "cereal",
            "bowl",
            "sandwich",
            "blt",
            "apple",
            "meal",
            "food",
        ],
    );
    let had_nonfood = has_any(
        &lower,
        &["meeting", "call", "appointment", "class", "workout", "session"],
    );
    dump.food = ate || (had && food_cue) || (food_page && had && !had_nonfood);
    dump
}

/// Obvious dumps (spend / ate / todo / study session) — skip MLX.
pub fn fast_life_write_intents(text: &str) -> Vec<(&'static str, String)> {
    fast_life_write_intents_ctx(text, None)
}

pub fn fast_life_write_intents_ctx(
    text: &str,
    ui_context: Option<&str>,
) -> Vec<(&'static str, String)> {
    let lower = text.trim().to_ascii_lowercase();
    if lower.is_empty() || looks_like_organize(&lower) {
        return Vec::new();
    }
    let pot_jobs = parse_money_pots(&lower);
    if is_life_readish(&lower) && pot_jobs.is_none() {
        return Vec::new();
    }
    let dump = classify_life_dump(text, ui_context);
    let page = page_hint(ui_context);
    let mut out = Vec::new();
    if let Some(inputs) = pot_jobs {
        for input in inputs {
            out.push(("money.pot", input));
        }
    } else if let Some(input) = parse_money_log(&lower, dump.money || on_page(&page, "money")) {
        out.push(("money.log", input));
    }
    if let Some(inputs) = parse_food_logs(&lower, dump.food) {
        for input in inputs {
            out.push(("fitness.log_food", input));
        }
    }
    if let Some(input) = parse_workout_log(&lower) {
        out.push(("fitness.log_workout", input));
    }
    if let Some(input) = parse_todo_add(&lower, text) {
        out.push(("todo.add", input));
    }
    if let Some(input) = parse_study_log(&lower) {
        out.push(("study.log_session", input));
    }
    if let Some(input) = parse_spark_fast(&lower, text) {
        out.push(("save_spark", input));
    }
    if let Some(input) = parse_fridge_add(&lower) {
        out.push(("fitness.fridge", input));
    }
    out
}

fn count_easy_todos(lower: &str) -> usize {
    if !(has_any(lower, &["todo", "to-do", "to do", "to dos", "tasks"])
        || lower.contains("to dos"))
    {
        return 0;
    }
    for (word, n) in [
        ("three ", 3usize),
        ("3 ", 3),
        ("two ", 2),
        ("2 ", 2),
        ("a couple", 2),
        ("one ", 1),
    ] {
        if lower.contains(word) {
            return n;
        }
    }
    2
}

struct StringedSpec {
    when: String,
    wants_slot: bool,
    spark_n: usize,
    todo_n: usize,
    wants_study_write: bool,
}

fn fast_stringed_spec(text: &str, _ui_context: Option<&str>) -> Option<StringedSpec> {
    let lower = text.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return None;
    }
    let wants_slot = has_any(
        &lower,
        &[
            "find a slot",
            "a slot for",
            "free slot",
            "find time",
            "find me a slot",
        ],
    ) || (has_word(&lower, "slot") && has_any(&lower, &["spark", "todo", "to do"]));
    let spark_n = if has_word(&lower, "spark") || has_word(&lower, "sparks") {
        1
    } else {
        0
    };
    let todo_n = count_easy_todos(&lower);
    let wants_study_write = (has_word(&lower, "study")
        || lower.contains("assignment")
        || has_word(&lower, "topic"))
        && has_any(&lower, &["add", "create", "first two", "assignments", "topic"]);
    if !wants_slot && !wants_study_write {
        return None;
    }
    if !wants_slot && spark_n == 0 && todo_n == 0 && !wants_study_write {
        return None;
    }
    let when = parse_when_label(&lower)
        .unwrap_or(if wants_slot || wants_study_write {
            "tomorrow"
        } else {
            "today"
        })
        .to_string();
    Some(StringedSpec {
        when,
        wants_slot,
        spark_n,
        todo_n,
        wants_study_write,
    })
}

fn stringed_read_jobs(spec: &StringedSpec) -> Vec<(&'static str, String)> {
    let mut jobs: Vec<(&'static str, String)> = Vec::new();
    if spec.wants_slot {
        jobs.push((
            "calendar.look",
            json!({
                "when": spec.when,
                "focus": "free",
                "duration_minutes": 60
            })
            .to_string(),
        ));
    } else if spec.wants_study_write {
        jobs.push((
            "calendar.look",
            json!({ "when": spec.when, "focus": "events" }).to_string(),
        ));
    }
    if spec.spark_n > 0 {
        jobs.push(("list_sparks", json!({ "status": "active" }).to_string()));
    }
    if spec.todo_n > 0 {
        jobs.push(("todo.list", "{}".into()));
    }
    if spec.wants_study_write {
        jobs.push((
            "study.look",
            json!({ "what": "topics", "limit": 40 }).to_string(),
        ));
        jobs.push((
            "study.look",
            json!({ "what": "assignments", "limit": 40 }).to_string(),
        ));
    }
    jobs
}

/// Deterministic multi-tool plan for stringed asks — reads first; writes use those rows.
pub fn fast_stringed_plan(
    text: &str,
    ui_context: Option<&str>,
) -> Option<Vec<(&'static str, String)>> {
    let spec = fast_stringed_spec(text, ui_context)?;
    let jobs = stringed_read_jobs(&spec);
    if jobs.len() < 2 {
        return None;
    }
    Some(jobs)
}

fn parse_json_items(output: &str) -> Vec<Value> {
    match serde_json::from_str::<Value>(output.trim()) {
        Ok(Value::Array(a)) => a,
        Ok(Value::Object(o)) => ["items", "todos", "sparks", "events", "assignments", "topics"]
            .iter()
            .find_map(|k| o.get(*k).and_then(|v| v.as_array()).cloned())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn short_title(s: &str, max: usize) -> String {
    let line = s.lines().next().unwrap_or(s).trim();
    if line.is_empty() {
        return String::new();
    }
    let count = line.chars().count();
    if count <= max {
        return line.to_string();
    }
    format!("{}…", line.chars().take(max.saturating_sub(1)).collect::<String>())
}

fn pick_spark_titles(output: &str, n: usize) -> Vec<String> {
    let mut items = parse_json_items(output);
    items.retain(|s| {
        !matches!(
            s.get("status").and_then(|st| st.as_str()).unwrap_or("active"),
            "archived" | "done" | "deleted"
        )
    });
    items.sort_by(|a, b| {
        let ua = a.get("updated_at").and_then(|v| v.as_i64()).unwrap_or(0);
        let ub = b.get("updated_at").and_then(|v| v.as_i64()).unwrap_or(0);
        ub.cmp(&ua)
    });
    items
        .into_iter()
        .filter_map(|s| {
            let c = s.get("content")?.as_str()?.trim();
            if c.is_empty() {
                None
            } else {
                Some(short_title(c, 60))
            }
        })
        .take(n)
        .collect()
}

fn pick_todo_titles(output: &str, n: usize, easy: bool) -> Vec<String> {
    let mut items = parse_json_items(output);
    items.retain(|t| {
        !t.get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .eq_ignore_ascii_case("completed")
    });
    let easy_only: Vec<Value> = items
        .iter()
        .filter(|t| {
            t.get("priority")
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .eq_ignore_ascii_case("low")
        })
        .cloned()
        .collect();
    let mut pool = if easy && !easy_only.is_empty() {
        easy_only
    } else {
        items
    };
    pool.sort_by_key(|t| {
        let overdue = t.get("overdue").and_then(|v| v.as_bool()) == Some(true);
        let deadline = t
            .get("deadline")
            .and_then(|d| d.as_str())
            .unwrap_or("9999-99-99")
            .to_string();
        (!overdue, deadline)
    });
    pool.into_iter()
        .filter_map(|t| {
            t.get("title")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned)
        })
        .take(n)
        .collect()
}

fn study_event_title(look_output: &str) -> Option<String> {
    let v: Value = serde_json::from_str(look_output.trim()).ok()?;
    let events = v.get("events").and_then(|e| e.as_array())?;
    let study = events.iter().find(|e| {
        e.get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .contains("study")
    });
    study
        .or(events.first())
        .and_then(|e| e.get("title").and_then(|t| t.as_str()))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

/// High-precision calendar shortcut: look / clock-pin only. Organize never matches.
pub fn fast_calendar_intent(text: &str) -> Option<(&'static str, String)> {
    let lower = text.trim().to_ascii_lowercase();
    if lower.is_empty() || looks_like_organize(&lower) {
        return None;
    }
    if fast_stringed_plan(text, None).is_some()
        || is_multi_intent(text, None)
        || fast_life_look_intent(text).is_some()
        || mixed_life_intent(&lower)
        || classify_life_dump(text, None).blocks_complete()
        || !fast_life_write_intents(text).is_empty()
    {
        return None;
    }
    if has_look_cue(&lower) {
        if looks_like_look_free(&lower) {
            return Some(("calendar.look", look_free_input(&lower)));
        }
        if looks_like_look_work(&lower) {
            return Some(("calendar.look", look_work_input(&lower)));
        }
        return Some(("calendar.look", look_agenda_input(&lower)));
    }
    if looks_like_clock_pin(&lower) {
        return Some(("calendar.pin", pin_fast_input(text, &lower)));
    }
    None
}

fn model_user_text(text: &str, ui_context: Option<&str>) -> String {
    match ui_context.map(str::trim).filter(|s| !s.is_empty()) {
        Some(ctx) => format!("{text}\n\n<ui_context>\n{ctx}\n</ui_context>"),
        None => text.to_string(),
    }
}

fn tools_for_turn(
    state: &AppState,
    text: &str,
    ui_context: Option<&str>,
    conv_kind: &str,
    chat_mode: ChatMode,
) -> Vec<Value> {
    let all = state.plugins.openai_tools();
    let mut selectors = selectors_for_turn(text, ui_context, chat_mode);
    if conv_kind == "research" && !selectors.iter().any(|s| *s == "research.") {
        selectors.push("research.");
    }
    if selectors.is_empty() {
        return Vec::new();
    }
    let filtered: Vec<Value> = all
        .iter()
        .filter(|t| {
            let name = t
                .pointer("/function/name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            selector_matches(name, &selectors)
        })
        .cloned()
        .collect();
    if filtered.is_empty() {
        all
    } else {
        filtered
    }
}

fn selector_matches(name: &str, selectors: &[&str]) -> bool {
    selectors.iter().any(|s| {
        if s.ends_with('.') {
            name.starts_with(s)
        } else {
            name == *s
        }
    })
}

/// Whether this user turn should enter the native tool loop (vs streaming chat).
pub fn turn_wants_tools(text: &str, ui_context: Option<&str>) -> bool {
    !tool_prefixes_for_turn(text, ui_context).is_empty()
}

/// Short chitchat with no domain signal — Llama may answer if Qwen is not resident.
/// Longer or messy text goes to Qwen even without keywords.
pub fn is_trivial_chat(text: &str, ui_context: Option<&str>) -> bool {
    let trimmed = text.trim();
    if trimmed.len() > 120 || trimmed.matches(',').count() >= 2 {
        return false;
    }
    if classify_life_dump(text, ui_context).blocks_complete() {
        return false;
    }
    tool_prefixes_for_turn(text, ui_context).is_empty()
}

/// Selectors to attach. Talk → none. Tool → matched families, or full life kit
/// when the turn has no keywords / looks like a mixed dump.
pub fn selectors_for_turn(
    text: &str,
    ui_context: Option<&str>,
    chat_mode: ChatMode,
) -> Vec<&'static str> {
    if chat_mode == ChatMode::Talk {
        return Vec::new();
    }
    if looks_like_goal_intake(text) {
        return vec!["goal.intake", "goal.look"];
    }
    let mut out = tool_prefixes_for_turn(text, ui_context);
    let dump = classify_life_dump(text, ui_context);
    if dump.blocks_complete() {
        if dump.money && !out.contains(&"money.") {
            out.push("money.");
        }
        if dump.food && !out.contains(&"fitness.") {
            out.push("fitness.");
        }
        if dump.curriculum && !out.contains(&"study.") {
            out.push("study.");
        }
        return out;
    }
    // Keyword hits keep that family. Long/messy dumps get the full kit so Qwen
    // can split multi-intent. Short trivia attaches nothing — that's chat.
    if out.is_empty() && looks_like_dump(text) {
        for selector in crate::skills::prefixes_for_turn(text, ui_context) {
            if !out.contains(&selector) {
                out.push(selector);
            }
        }
    }
    out
}

/// Goal sentences like V6 must not drag in the full fitness write kit.
pub fn looks_like_goal_intake(text: &str) -> bool {
    let lower = text.trim().to_ascii_lowercase();
    let wants = has_any(&lower, &["i want to", "my goal", "my goals"]);
    let horizon = has_any(
        &lower,
        &[
            "by ",
            "end of",
            "november",
            "december",
            "january",
            "february",
            "march",
            "april",
            "june",
            "july",
            "august",
            "september",
            "october",
            "deadline",
        ],
    );
    wants && horizon
}

pub(crate) fn looks_like_dump(text: &str) -> bool {
    let t = text.trim();
    t.len() > 100 || t.matches(',').count() >= 2
}

fn selector_family(selector: &str) -> &'static str {
    match selector {
        "calendar." | "lifestyle." | "dream." | "work." => "calendar",
        "todo." => "todo",
        "study." => "study",
        "fitness." => "fitness",
        "money." => "money",
        "docs." => "docs",
        "socials." => "socials",
        "research." => "research",
        "save_spark" | "list_sparks" | "update_spark" => "spark",
        _ => "other",
    }
}

pub fn is_multi_intent(text: &str, ui_context: Option<&str>) -> bool {
    let prefixes = tool_prefixes_for_turn(text, ui_context);
    let mut families: Vec<&str> = Vec::new();
    for p in &prefixes {
        let f = selector_family(p);
        if !families.contains(&f) {
            families.push(f);
        }
    }
    families.len() >= 2
}

/// Tool name prefixes / exact names to attach. Empty → conversation, no tools.
pub fn tool_prefixes_for_turn(text: &str, ui_context: Option<&str>) -> Vec<&'static str> {
    crate::skills::prefixes_for_turn(text, ui_context)
}

pub(crate) fn has_any(lower: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| {
        if n.chars()
            .any(|c| c.is_whitespace() || matches!(c, '.' | '-' | '£' | ':' | '/'))
        {
            lower.contains(n)
        } else {
            has_word(lower, n)
        }
    })
}

pub(crate) fn has_word(lower: &str, word: &str) -> bool {
    lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|w| w == word)
}

pub(crate) fn has_actiony(lower: &str) -> bool {
    has_any(
        lower,
        &[
            "add", "create", "make", "log", "edit", "update", "delete", "remove", "show", "list",
            "search", "find", "open", "write", "schedule", "plan", "format", "save", "track",
            "record", "patch",
        ],
    )
}

pub(crate) fn looks_like_code_request(lower: &str) -> bool {
    has_any(
        lower,
        &[
            "write code",
            "help me code",
            "coder.run",
            "edit_file",
            "write_file",
            "in the repo",
            "in this project",
        ],
    ) || ((lower.contains("~/") || lower.contains("/users/"))
        && has_any(lower, &["edit", "write", "read", "open file"]))
}

pub(crate) fn looks_like_spark_save(lower: &str) -> bool {
    has_any(
        lower,
        &[
            "note to self",
            "save this idea",
            "save this thought",
            "jot this down",
            "brain dump",
        ],
    ) || lower.starts_with("spark:")
        || lower.contains("spark: ")
}

pub(crate) fn looks_like_spark_list(lower: &str) -> bool {
    has_any(lower, &["my sparks", "list sparks", "saved sparks", "spark list"])
        || (has_word(lower, "sparks") && has_actiony(lower))
}

/// True when this turn should expose only calendar/lifestyle tools.
#[cfg(test)]
fn calendar_only_tool_filter(text: &str, ui_context: Option<&str>) -> bool {
    let selectors = tool_prefixes_for_turn(text, ui_context);
    !selectors.is_empty()
        && selectors.iter().all(|s| {
            matches!(*s, "calendar." | "lifestyle." | "dream." | "work.")
        })
}

fn mixed_life_intent(lower: &str) -> bool {
    const CUES: &[&str] = &[
        "todo",
        "to-do",
        "to do",
        "task",
        "get done",
        "need to do",
        "deadline",
        "study",
        "exam",
        "assignment",
        "eat",
        "food",
        "calorie",
        "fridge",
        "workout",
        "climb",
        "weight",
        "money",
        "spend",
        "spent",
        "paid",
        "earned",
        "invoice",
        "ate",
        "expense",
        "document",
        "docs",
        "sheet",
        "spark",
        "research",
        "social",
        "linkedin",
        "twitter",
        "weekly review",
        "posting",
    ];
    CUES.iter().any(|c| lower.contains(c))
}

pub(crate) fn looks_like_organize(lower: &str) -> bool {
    if     lower.contains("plan my week")
        || lower.contains("plan the week")
        || lower.contains("plan out my week")
        || lower.contains("plan next week")
        || lower.contains("plan my day")
        || lower.contains("plan the day")
        || lower.contains("plan out my day")
    {
        return true;
    }
    let verb = [
        "book ",
        "schedule ",
        "make time",
        "fit ",
        "find time",
        "organise",
        "organize",
    ]
    .iter()
    .any(|v| lower.contains(v));
    if !verb {
        return false;
    }
    lower.contains("this week")
        || lower.contains("next week")
        || lower.contains("sessions")
        || lower.contains(" times")
        || (lower.contains("once") && lower.contains("week"))
}

pub(crate) fn has_look_cue(lower: &str) -> bool {
    const CUES: &[&str] = &[
        "what's on",
        "whats on",
        "what is on",
        "what's happening",
        "whats happening",
        "what is happening",
        "what's planned",
        "whats planned",
        "what is planned",
        "on my calendar",
        "my calendar",
        "my schedule",
        "my agenda",
        "my plans",
        "anything",
        "show me",
        "free time",
        "free slot",
        "open slot",
        "any free",
        "when am i free",
        "am i free",
        "when am i working",
        "when do i work",
        "what time do i work",
        "what are my work hours",
        "when do i finish work",
        "when does work end",
        "when am i at work",
    ];
    if CUES.iter().any(|c| lower.contains(c)) {
        return true;
    }
    if lower.contains("do i have") && has_calendar_anchor(lower) {
        return true;
    }
    if has_plans_noun(lower) {
        return true;
    }
    let ask = lower.contains("give me")
        || lower.contains("show me")
        || lower.starts_with("all ")
        || lower.contains("list ");
    ask && has_calendar_anchor(lower) && !lower.contains("plan ")
}

fn has_calendar_anchor(lower: &str) -> bool {
    lower.contains("week")
        || lower.contains("calendar")
        || lower.contains("agenda")
        || lower.contains("schedule")
        || lower.contains("slot")
        || lower.contains("free")
        || lower.contains("today")
        || lower.contains("tomorrow")
        || lower.contains("weekend")
        || parse_when_label(lower).is_some()
}

fn has_plans_noun(lower: &str) -> bool {
    if lower.contains("planning") || lower.contains("plan my") || lower.contains("plan the") {
        return false;
    }
    let t = lower
        .trim()
        .trim_end_matches(['?', '.', '!'])
        .trim();
    t == "plans"
        || t.ends_with(" plans")
        || t.starts_with("plans ")
        || t.contains(" plans ")
        || t.contains("agenda")
}

fn looks_like_look_free(lower: &str) -> bool {
    lower.contains("when am i free")
        || lower.contains("free time")
        || lower.contains("free slot")
        || lower.contains("open slot")
        || lower.contains("any free")
        || lower.contains("am i free")
}

fn looks_like_look_work(lower: &str) -> bool {
    lower.contains("when am i working")
        || lower.contains("when do i work")
        || lower.contains("what time do i work")
        || lower.contains("what are my work hours")
        || lower.contains("when do i finish work")
        || lower.contains("when does work end")
        || lower.contains("when am i at work")
}

pub(crate) fn looks_like_clock_pin(lower: &str) -> bool {
    if !has_clock_token(lower) {
        return false;
    }
    ["dentist", "doctor", "meeting", "appointment", "call", "lunch", "interview"]
        .iter()
        .any(|w| lower.contains(w))
}

fn has_clock_token(lower: &str) -> bool {
    extract_clock_token(lower).is_some()
}

fn extract_clock_token(lower: &str) -> Option<String> {
    let chars: Vec<char> = lower.chars().collect();
    let n = chars.len();
    let mut i = 0;
    while i < n {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < n && chars[i].is_ascii_digit() {
                i += 1;
            }
            let mut end = i;
            if i < n && chars[i] == ':' {
                i += 1;
                let m0 = i;
                while i < n && chars[i].is_ascii_digit() {
                    i += 1;
                }
                if i - m0 != 2 {
                    continue;
                }
                end = i;
            }
            let mut j = end;
            while j < n && chars[j].is_whitespace() {
                j += 1;
            }
            let ampm = if j + 1 < n {
                let a = chars[j];
                let p = chars[j + 1];
                if (a == 'a' || a == 'p') && p == 'm' {
                    Some(if a == 'p' { "pm" } else { "am" })
                } else {
                    None
                }
            } else {
                None
            };
            let num: String = chars[start..end].iter().collect();
            if let Some(ap) = ampm {
                return Some(format!("{num}{ap}"));
            }
            if num.contains(':') {
                return Some(num);
            }
        } else {
            i += 1;
        }
    }
    None
}

fn look_when(lower: &str) -> &'static str {
    if let Some(when) = parse_when_label(lower) {
        return when;
    }
    if looks_like_look_work(lower) || has_plans_noun(lower) {
        return "this_week";
    }
    "today"
}

fn look_agenda_input(lower: &str) -> String {
    json!({ "when": look_when(lower), "focus": "events" }).to_string()
}

fn look_work_input(lower: &str) -> String {
    let when = parse_when_label(lower).unwrap_or("this_week");
    json!({ "when": when, "focus": "work" }).to_string()
}

fn look_free_input(lower: &str) -> String {
    let mut payload = json!({ "when": look_when(lower), "focus": "free" });
    if let Some(mins) = parse_fast_duration_minutes(lower) {
        payload["duration_minutes"] = json!(mins);
    } else {
        payload["duration_minutes"] = json!(60);
    }
    payload.to_string()
}

fn parse_fast_duration_minutes(lower: &str) -> Option<u32> {
    if let Some(idx) = lower.find(" hour") {
        let prefix = &lower[..idx];
        let num = prefix
            .rsplit(|c: char| !c.is_ascii_digit() && c != '.')
            .next()?;
        let h: f64 = num.parse().ok()?;
        return Some((h * 60.0).round().max(15.0) as u32);
    }
    if let Some(idx) = lower.find(" hr") {
        let prefix = &lower[..idx];
        let num = prefix
            .rsplit(|c: char| !c.is_ascii_digit() && c != '.')
            .next()?;
        let h: f64 = num.parse().ok()?;
        return Some((h * 60.0).round().max(15.0) as u32);
    }
    for marker in [" minutes", " minute", " mins", " min"] {
        if let Some(idx) = lower.find(marker) {
            let prefix = &lower[..idx];
            let num = prefix.rsplit(|c: char| !c.is_ascii_digit()).next()?;
            return num.parse().ok();
        }
    }
    None
}

fn pin_fast_input(original: &str, lower: &str) -> String {
    let title = if lower.contains("dentist") {
        "Dentist"
    } else if lower.contains("doctor") {
        "Doctor"
    } else if lower.contains("lunch") {
        "Lunch"
    } else if lower.contains("interview") {
        "Interview"
    } else if lower.contains("appointment") {
        "Appointment"
    } else if lower.contains("call") {
        "Call"
    } else {
        "Meeting"
    };
    let start = extract_pin_start(original, lower).unwrap_or_else(|| "tomorrow 09:00".into());
    json!({
        "action": "create",
        "title": title,
        "start": start,
    })
    .to_string()
}

fn extract_pin_start(_original: &str, lower: &str) -> Option<String> {
    let day = if lower.contains("tomorrow") {
        "tomorrow"
    } else if lower.contains("today") {
        "today"
    } else if lower.contains("friday") {
        "friday"
    } else if lower.contains("monday") {
        "monday"
    } else if lower.contains("tuesday") {
        "tuesday"
    } else if lower.contains("wednesday") {
        "wednesday"
    } else if lower.contains("thursday") {
        "thursday"
    } else if lower.contains("saturday") {
        "saturday"
    } else if lower.contains("sunday") {
        "sunday"
    } else {
        "today"
    };
    let clock = extract_clock_token(lower)?;
    Some(format!("{day} {clock}"))
}

fn parse_leading_float(s: &str) -> Option<f64> {
    let s = s.trim_start();
    let end = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());
    if end == 0 {
        return None;
    }
    s[..end].parse().ok().filter(|n: &f64| *n > 0.0)
}

fn parse_money_amount(lower: &str) -> Option<f64> {
    if let Some(i) = lower.find('£') {
        if let Some(n) = parse_leading_float(&lower[i + '£'.len_utf8()..]) {
            return Some(n);
        }
    }
    for marker in [" pounds", " quid", " gbp"] {
        if let Some(i) = lower.find(marker) {
            let prefix = &lower[..i];
            if let Some(num) = prefix
                .rsplit(|c: char| !c.is_ascii_digit() && c != '.')
                .next()
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|n| *n > 0.0)
            {
                return Some(num);
            }
        }
    }
    for verb in ["spent ", "paid ", "earned ", "cost "] {
        if let Some(i) = lower.find(verb) {
            let rest = &lower[i + verb.len()..];
            if rest.starts_with("too ") || rest.starts_with("a lot") {
                continue;
            }
            if let Some(n) = parse_leading_float(rest) {
                let after = rest
                    .trim_start()
                    .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.')
                    .trim_start();
                if after.starts_with("min")
                    || after.starts_with("hour")
                    || after.starts_with("am")
                    || after.starts_with("pm")
                    || after.starts_with(':')
                {
                    continue;
                }
                return Some(n);
            }
        }
    }
    None
}

fn money_category(desc: &str) -> &'static str {
    if has_any(desc, &["rent", "mortgage"]) {
        "rent"
    } else if has_any(desc, &["lunch", "dinner", "coffee", "food", "market", "grocer", "cafe", "eat"]) {
        "food"
    } else if has_any(desc, &["uber", "bus", "train", "petrol", "taxi", "transport"]) {
        "transport"
    } else if has_any(desc, &["phone", "broadband", "wifi", "internet"]) {
        "phone"
    } else if has_any(desc, &["electric", "gas", "water", "utilit"]) {
        "utilities"
    } else if has_any(desc, &["shop", "amazon", "clothes"]) {
        "shop"
    } else {
        "general"
    }
}

const NOT_POT_NAMES: &[&str] = &[
    "rent", "lunch", "dinner", "market", "grocery", "groceries", "food", "bill", "bills", "phone",
    "uber", "coffee", "taxi", "bus",
];

fn looks_like_pot_dump(lower: &str) -> bool {
    has_any(
        lower,
        &[
            " pot",
            "pot ",
            "pots",
            "savings split",
            "savings pots",
            "savings pot",
        ],
    ) || (has_any(lower, &["put ", "moved ", "move ", "added ", "add "])
        && has_any(lower, &[" into ", " in ", " to "])
        && parse_money_amount(lower).is_some())
}

fn clean_pot_name(raw: &str) -> String {
    raw.replace(['.', ',', ';', ':', '!', '?'], " ")
        .split_whitespace()
        .filter(|w| {
            !matches!(
                *w,
                "pot"
                    | "pots"
                    | "the"
                    | "my"
                    | "a"
                    | "an"
                    | "is"
                    | "was"
                    | "to"
                    | "into"
                    | "in"
                    | "put"
                    | "moved"
                    | "move"
                    | "add"
                    | "added"
                    | "savings"
                    | "split"
                    | "of"
                    | "and"
                    | "with"
                    | "for"
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn find_amount_spans(s: &str) -> Vec<(usize, usize, f64)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if !s.is_char_boundary(i) {
            i += 1;
            continue;
        }
        let rest = &s[i..];
        if rest.starts_with('£') {
            let num = &rest['£'.len_utf8()..];
            if let Some(n) = parse_leading_float(num) {
                let nend = num
                    .find(|c: char| !c.is_ascii_digit() && c != '.')
                    .unwrap_or(num.len());
                let end = i + '£'.len_utf8() + nend;
                out.push((i, end, n));
                i = end;
                continue;
            }
        }
        let ch = rest.chars().next().unwrap_or(' ');
        if ch.is_ascii_digit() {
            let prev_ok = i == 0
                || s[..i].ends_with(' ')
                || s[..i].ends_with(',')
                || s[..i].ends_with(':');
            if prev_ok {
                if let Some(n) = parse_leading_float(rest) {
                    if !(1900.0..=2100.0).contains(&n) {
                        let nend = rest
                            .find(|c: char| !c.is_ascii_digit() && c != '.')
                            .unwrap_or(rest.len());
                        out.push((i, i + nend, n));
                        i += nend;
                        continue;
                    }
                }
            }
        }
        i += ch.len_utf8();
    }
    out
}

fn split_first_amount(s: &str) -> Option<(f64, &str)> {
    let spans = find_amount_spans(s);
    let (start, end, n) = spans.first().copied()?;
    if start >= 40 {
        return None;
    }
    Some((n, s.get(end..).unwrap_or("").trim_start()))
}

fn strip_pot_prep(s: &str) -> &str {
    for p in ["into ", "in the ", "in my ", "in ", "to the ", "to my ", "to "] {
        if let Some(rest) = s.strip_prefix(p) {
            return rest;
        }
    }
    s
}

fn next_char_idx(s: &str, mut i: usize) -> usize {
    i = i.saturating_add(1);
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i.min(s.len())
}

fn parse_put_into_pots(lower: &str) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    let mut idx = 0;
    while idx < lower.len() {
        if !lower.is_char_boundary(idx) {
            idx += 1;
            continue;
        }
        let rest = &lower[idx..];
        let verb = ["put ", "moved ", "move ", "added ", "add "]
            .iter()
            .filter_map(|v| rest.find(v).map(|i| (i, v.len())))
            .min_by_key(|(i, _)| *i);
        let Some((vi, vl)) = verb else {
            break;
        };
        let after_start = idx + vi + vl;
        if after_start > lower.len() || !lower.is_char_boundary(after_start) {
            idx = next_char_idx(lower, idx + vi);
            continue;
        }
        let after = &lower[after_start..];
        let Some((amount, after_amt)) = split_first_amount(after) else {
            idx = next_char_idx(lower, after_start);
            continue;
        };
        let name_src = strip_pot_prep(after_amt);
        let chunk = name_src
            .split(|c| matches!(c, ',' | ';' | '.'))
            .next()
            .unwrap_or(name_src);
        let name = clean_pot_name(chunk);
        if !name.is_empty() {
            out.push((name, amount));
        }
        idx = after_start
            + find_amount_spans(after)
                .first()
                .map(|(_, end, _)| *end)
                .unwrap_or(1);
        if !lower.is_char_boundary(idx) {
            idx = next_char_idx(lower, idx);
        }
    }
    out
}

fn parse_name_amount_pots(lower: &str) -> Vec<(String, f64)> {
    let mut s = lower;
    for prefix in [
        "savings pots:",
        "savings pot:",
        "savings split:",
        "pots:",
        "pot:",
        "split:",
    ] {
        if let Some(i) = s.find(prefix) {
            s = s[i + prefix.len()..].trim();
            break;
        }
    }
    let amounts = find_amount_spans(s);
    if amounts.is_empty() {
        return Vec::new();
    }
    let mut pairs = Vec::new();
    let mut prev_end = 0;
    for (start, end, val) in amounts {
        if !s.is_char_boundary(start) || !s.is_char_boundary(prev_end) {
            prev_end = end;
            continue;
        }
        let raw = s.get(prev_end..start).unwrap_or("").trim();
        let name = clean_pot_name(raw);
        if !name.is_empty() {
            pairs.push((name, val));
        }
        prev_end = end;
    }
    pairs
}

fn pot_jobs_json(pairs: Vec<(String, f64)>, mode: &str) -> Option<Vec<String>> {
    let pairs: Vec<(String, f64)> = pairs
        .into_iter()
        .filter(|(n, a)| n.len() >= 2 && *a > 0.0)
        .collect();
    if pairs.is_empty() {
        return None;
    }
    Some(
        pairs
            .into_iter()
            .map(|(name, amount)| {
                json!({
                    "name": name,
                    "amount": amount,
                    "mode": mode,
                })
                .to_string()
            })
            .collect(),
    )
}

fn parse_money_pots(lower: &str) -> Option<Vec<String>> {
    if lower.contains('?') || lower.contains("how much") || lower.contains("what did i spend") {
        return None;
    }
    if !looks_like_pot_dump(lower) {
        return None;
    }
    let mentioned_pot = has_any(
        lower,
        &[" pot", "pot ", "pots", "savings pot", "savings pots", "savings split"],
    );
    let add_mode = has_any(lower, &["put ", "into ", "moved ", "move ", "added "]);
    let mode = if add_mode { "add" } else { "set" };
    let mut pairs = parse_put_into_pots(lower);
    if pairs.is_empty() {
        pairs = parse_name_amount_pots(lower);
    }
    pairs.retain(|(name, _)| {
        mentioned_pot || !NOT_POT_NAMES.iter().any(|n| name == *n)
    });
    pot_jobs_json(pairs, mode)
}

fn parse_money_log(lower: &str, force: bool) -> Option<String> {
    if lower.contains('?') || lower.contains("how much") || lower.contains("what did i spend") {
        return None;
    }
    let income = has_any(lower, &["earned", "got paid", "income", "salary", "freelance"]);
    let expense_verb = has_any(lower, &["spent", "paid", "expense", "cost me", "bought", "cost "]);
    let amount = parse_money_amount(lower)?;
    let was_amount = lower.contains(" was £")
        || lower.contains(" was $")
        || lower.contains(" is £")
        || (lower.contains(" was ") && (lower.contains('£') || lower.contains("pound")));
    let money_noun = has_any(
        lower,
        &[
            "rent",
            "bill",
            "bills",
            "mortgage",
            "subscription",
            "utilities",
            "phone",
            "grocery",
            "groceries",
        ],
    );
    if !force && !income && !expense_verb && !was_amount && !money_noun {
        return None;
    }
    let kind = if income { "income" } else { "expense" };
    let mut desc = String::new();
    for prep in [" at the ", " at ", " on ", " for "] {
        if let Some(i) = lower.find(prep) {
            let rest = lower[i + prep.len()..]
                .split(|c: char| matches!(c, ',' | ';' | '!'))
                .next()
                .unwrap_or("")
                .trim();
            let cleaned = rest
                .trim_end_matches(" today")
                .trim_end_matches(" yesterday")
                .trim()
                .trim_start_matches("the ")
                .trim();
            if cleaned.len() >= 2 && cleaned != "a" && !cleaned.starts_with('£') {
                desc = cleaned.to_string();
                break;
            }
        }
    }
    if desc.is_empty() {
        if let Some(i) = lower.find("bought ") {
            let rest = lower[i + 7..]
                .split(|c: char| matches!(c, ',' | '.' | ';'))
                .next()
                .unwrap_or("")
                .trim();
            let cleaned = rest
                .split(" for ")
                .next()
                .unwrap_or(rest)
                .trim()
                .trim_start_matches("a ")
                .trim_start_matches("an ")
                .trim_start_matches("some ");
            if cleaned.len() >= 2 {
                desc = cleaned.to_string();
            }
        }
    }
    if desc.is_empty() {
        for marker in [" was £", " was $", " is £", " was ", " is "] {
            if let Some(i) = lower.find(marker) {
                let left = lower[..i]
                    .trim()
                    .trim_start_matches("the ")
                    .trim_start_matches("my ")
                    .trim();
                if left.len() >= 2 {
                    desc = left.to_string();
                    break;
                }
            }
        }
    }
    if desc.is_empty() {
        desc = if income {
            "income".into()
        } else if money_noun {
            [
                "rent",
                "mortgage",
                "phone",
                "grocery",
                "groceries",
                "subscription",
                "bill",
            ]
            .iter()
            .find(|n| lower.contains(*n))
            .unwrap_or(&"expense")
            .to_string()
        } else {
            "expense".into()
        };
    }
    let category = money_category(&desc);
    Some(
        json!({
            "kind": kind,
            "description": desc,
            "amount": amount,
            "category": category,
        })
        .to_string(),
    )
}

fn estimate_food(name: &str) -> (f64, f64, f64, f64) {
    let n = name.to_ascii_lowercase();
    if n.contains("burrito") {
        (750.0, 30.0, 80.0, 28.0)
    } else if n.contains("pizza") {
        (800.0, 32.0, 90.0, 30.0)
    } else if n.contains("chicken") && n.contains("rice") {
        (650.0, 40.0, 70.0, 18.0)
    } else if n.contains("cereal")
        || n.contains("honey flake")
        || n.contains("cornflake")
        || ((n.contains("bowl") || n.contains("oat milk") || n.contains("milk"))
            && (n.contains("flake") || n.contains("cereal") || n.contains("oat")))
    {
        // Bowl of cereal with oat milk
        (320.0, 8.0, 58.0, 6.0)
    } else if n.contains("oat") || n.contains("porridge") {
        (350.0, 12.0, 55.0, 8.0)
    } else if n.contains("blt")
        || (n.contains("mortadella") || n.contains("mortedella"))
        || (n.contains("sandwich") && (n.contains("bacon") || n.contains("lettuce") || n.contains("tomato")))
    {
        // BLT-style sandwich (mortadella instead of bacon)
        (520.0, 22.0, 42.0, 28.0)
    } else if n.contains("salad") {
        (320.0, 18.0, 20.0, 16.0)
    } else if n.contains("pasta") {
        (700.0, 22.0, 95.0, 20.0)
    } else if n.contains("sandwich") || n.contains("toastie") {
        (480.0, 20.0, 48.0, 18.0)
    } else if n.contains("apple") {
        (95.0, 0.5, 25.0, 0.3)
    } else if n.contains("banana") {
        (105.0, 1.3, 27.0, 0.4)
    } else if n.contains("egg") {
        (220.0, 16.0, 2.0, 16.0)
    } else if n.contains("coffee") || n.contains("latte") {
        (180.0, 8.0, 18.0, 8.0)
    } else {
        (500.0, 20.0, 50.0, 20.0)
    }
}

fn food_meal_type(lower: &str) -> &'static str {
    if lower.contains("breakfast") {
        "breakfast"
    } else if lower.contains("lunch") {
        "lunch"
    } else if lower.contains("dinner") {
        "dinner"
    } else {
        "snack"
    }
}

fn clean_food_name(raw: &str) -> String {
    raw.trim()
        .trim_start_matches("a ")
        .trim_start_matches("an ")
        .trim_start_matches("some ")
        .trim_end_matches('.')
        .trim()
        .to_string()
}

fn split_trailing_and_item(part: &str) -> Option<(String, String)> {
    let lower = part.to_ascii_lowercase();
    for marker in [" and an ", " and a "] {
        if let Some(i) = lower.rfind(marker) {
            let right = part[i + marker.len()..].trim();
            let left = part[..i].trim();
            let words = right.split_whitespace().count();
            // Single fruit/side only — keep "X with Y and Z sauce" intact.
            if !left.is_empty()
                && words > 0
                && words <= 3
                && !right.to_ascii_lowercase().contains(" with ")
            {
                return Some((left.to_string(), right.to_string()));
            }
        }
    }
    None
}

fn split_food_items(rest: &str) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    for primary in rest
        .split(", then ")
        .flat_map(|p| p.split(" then "))
        .flat_map(|p| p.split(';'))
    {
        let primary = primary.trim().trim_start_matches(',').trim();
        if primary.is_empty() {
            continue;
        }
        if let Some((left, right)) = split_trailing_and_item(primary) {
            chunks.push(left);
            chunks.push(right);
        } else {
            chunks.push(primary.to_string());
        }
    }
    chunks
        .into_iter()
        .map(|c| clean_food_name(&c))
        .filter(|c| c.len() >= 2 && c != "food" && c != "it")
        .collect()
}

/// Parse one or more meals from an eat/had dump. Returns JSON inputs for fitness.log_food.
fn parse_food_logs(lower: &str, force: bool) -> Option<Vec<String>> {
    if lower.contains("should") || lower.contains("what did i eat") {
        return None;
    }
    let mealish = has_any(lower, &["breakfast", "lunch", "dinner", "snack"]);
    let food_ctx = force
        || mealish
        || has_any(
            lower,
            &[
                "food",
                "calorie",
                "calories",
                "meal",
                "ate",
                "eaten",
                "cereal",
                "bowl",
                "sandwich",
                "blt",
            ],
        );
    let start = if let Some(i) = lower.find("i ate ") {
        i + 6
    } else if let Some(i) = lower.find("ate ") {
        i + 4
    } else if let Some(i) = lower.find("i had ") {
        // "i had …" is enough when this is clearly food (page cue or meal words).
        if food_ctx || mealish || force {
            i + 6
        } else {
            return None;
        }
    } else if food_ctx {
        if let Some(i) = lower.find("had ") {
            i + 4
        } else {
            return None;
        }
    } else if let Some(i) = lower.find("log food ") {
        i + 9
    } else if let Some(i) = lower.find("logged food ") {
        i + 12
    } else {
        return None;
    };
    let mut rest = &lower[start..];
    for ender in [
        " for breakfast",
        " for lunch",
        " for dinner",
        " for snack",
        " today",
        " yesterday",
        " and i ",
        " and spent",
        " and paid",
        " and earned",
        "!",
        "?",
    ] {
        if let Some(i) = rest.find(ender) {
            rest = &rest[..i];
        }
    }
    rest = rest.trim().trim_end_matches('.');
    let names = split_food_items(rest);
    if names.is_empty() {
        return None;
    }
    let meal_type = food_meal_type(lower);
    Some(
        names
            .into_iter()
            .map(|name| {
                let (calories, protein, carbs, fat) = estimate_food(&name);
                json!({
                    "name": name,
                    "calories": calories,
                    "protein": protein,
                    "carbs": carbs,
                    "fat": fat,
                    "quantity": 1,
                    "unit": "serving",
                    "meal_type": meal_type,
                })
                .to_string()
            })
            .collect(),
    )
}

fn parse_workout_sets(lower: &str) -> Vec<Value> {
    let mut sets = Vec::new();
    for chunk in lower.split(|c: char| matches!(c, ',' | ';' | '&')) {
        for chunk in chunk.split(" and ") {
            if let Some((exercise, reps, weight)) = parse_one_set(chunk.trim()) {
                let mut set = json!({ "exercise": exercise, "reps": reps });
                if let Some(w) = weight {
                    set["weight"] = json!(w);
                }
                sets.push(set);
            }
        }
    }
    sets
}

fn parse_one_set(chunk: &str) -> Option<(String, i64, Option<f64>)> {
    let x_at = chunk.find('x').or_else(|| chunk.find('X'))?;
    let before = chunk[..x_at].trim();
    let a = before
        .rsplit(|c: char| !c.is_ascii_digit())
        .next()
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|n| *n > 0)?;
    let after_x = chunk[x_at + 1..].trim_start();
    let b_end = after_x
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after_x.len());
    if b_end == 0 {
        return None;
    }
    let b: i64 = after_x[..b_end].parse().ok().filter(|n| *n > 0)?;
    let rest = after_x[b_end..].trim_start();
    let kg_immediate = rest.starts_with("kg");
    let weight = if kg_immediate {
        Some(b as f64)
    } else {
        parse_leading_float(rest.trim_start_matches("at ").trim_start_matches('@').trim_start())
            .filter(|w| *w >= 1.0)
    };
    let reps = if kg_immediate || b > 40 {
        a
    } else {
        b
    };
    let ex_before = before
        .trim_end_matches(|c: char| c.is_ascii_digit() || c.is_whitespace())
        .trim()
        .trim_start_matches("i did ")
        .trim_start_matches("did ")
        .trim();
    let ex_after = rest
        .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.')
        .trim_start()
        .trim_start_matches("kg")
        .trim_start_matches("kgs")
        .trim_start_matches("at ")
        .trim();
    let exercise = if ex_before.len() >= 2 {
        title_case_words(ex_before)
    } else if ex_after.len() >= 2
        && !ex_after.starts_with("today")
        && !ex_after.starts_with("workout")
    {
        title_case_words(
            &ex_after
                .split_whitespace()
                .take(3)
                .collect::<Vec<_>>()
                .join(" "),
        )
    } else {
        "Exercise".into()
    };
    Some((exercise, reps, weight))
}

fn title_case_words(s: &str) -> String {
    s.split_whitespace()
        .filter(|w| !matches!(*w, "a" | "the" | "at" | "my"))
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => format!("{}{}", f.to_ascii_uppercase(), c.as_str()),
                None => String::new(),
            }
        })
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_workout_log(lower: &str) -> Option<String> {
    let workoutish = has_word(lower, "workout")
        || lower.contains("workouts")
        || has_any(
            lower,
            &[
                "i lifted",
                "i trained",
                "at the gym",
                "log workout",
                "logged a workout",
                "i worked out",
            ],
        );
    if !workoutish {
        return None;
    }
    let sets = parse_workout_sets(lower);
    let name = if lower.contains("push") {
        "Push"
    } else if lower.contains("pull") {
        "Pull"
    } else if lower.contains("leg") {
        "Legs"
    } else {
        "Workout"
    };
    Some(json!({ "name": name, "sets": sets }).to_string())
}

fn parse_todo_add(lower: &str, original: &str) -> Option<String> {
    let idx = if let Some(i) = lower.find("remind me to ") {
        i + 13
    } else if let Some(i) = lower.find("add a todo ") {
        i + 11
    } else if let Some(i) = lower.find("add todo ") {
        i + 9
    } else if let Some(i) = lower.find("add a task ") {
        i + 11
    } else if let Some(i) = lower.find("add task ") {
        i + 9
    } else if let Some(i) = lower.find("todo: ") {
        i + 6
    } else if let Some(i) = lower.find("to-do: ") {
        i + 7
    } else {
        return None;
    };
    let rest = original.get(idx..).unwrap_or("").trim();
    let title = rest
        .trim_end_matches(['.', '!', '?'])
        .trim()
        .trim_start_matches(':')
        .trim();
    if title.len() < 2 {
        return None;
    }
    Some(json!({ "title": title }).to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurriculumAssignment {
    pub title: String,
    pub kind: &'static str,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyCurriculum {
    pub topic: String,
    pub subject: Option<String>,
    pub assignments: Vec<CurriculumAssignment>,
}

/// Parse Cisco-style / module syllabi into one topic + Module Quiz assignments + Final Exam.
pub fn parse_study_curriculum(text: &str) -> Option<StudyCurriculum> {
    let lines: Vec<&str> = text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    if lines.len() < 2 {
        return None;
    }

    let mut modules: Vec<(u32, String, Vec<String>)> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("module ") {
            let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = num_str.parse::<u32>() {
                let title = line
                    .split_once(':')
                    .map(|(_, t)| t.trim().trim_end_matches('.').to_string())
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| format!("Module {n}"));
                let mut bullets = Vec::new();
                i += 1;
                while i < lines.len() {
                    let l = lines[i];
                    let ll = l.to_ascii_lowercase();
                    if ll.starts_with("module ")
                        || ll.starts_with("final assessment")
                        || ll.starts_with("final exam")
                        || ll.starts_with("work")
                        || ll.starts_with("sleep")
                    {
                        break;
                    }
                    if ll.starts_with("assignment:") {
                        // consumed later via module assignment scan
                        i += 1;
                        continue;
                    }
                    if ll.starts_with("topics:") {
                        let after = l.split_once(':').map(|(_, t)| t.trim()).unwrap_or("");
                        if !after.is_empty() {
                            bullets.push(after.to_string());
                        }
                        i += 1;
                        continue;
                    }
                    if !ll.starts_with("assignment") {
                        bullets.push(l.trim_end_matches('.').to_string());
                    }
                    i += 1;
                }
                modules.push((n, title, bullets));
                continue;
            }
        }
        i += 1;
    }

    let has_final = text.to_ascii_lowercase().contains("final exam")
        || text.to_ascii_lowercase().contains("final assessment");
    if modules.is_empty() && !has_final {
        return None;
    }
    // Need at least one graded item signal (Assignment: or Final Exam).
    let has_quiz = text.to_ascii_lowercase().contains("assignment:")
        || text.to_ascii_lowercase().contains("module quiz");
    if !has_quiz && !has_final {
        return None;
    }

    let mut assignments = Vec::new();
    for (n, title, bullets) in &modules {
        let notes = if bullets.is_empty() {
            Some(format!("Module {n}: {title}"))
        } else {
            Some(format!("Module {n}: {title}\n{}", bullets.join("\n")))
        };
        assignments.push(CurriculumAssignment {
            title: format!("Module {n} Quiz"),
            kind: "assignment",
            notes,
        });
    }
    if has_final {
        assignments.push(CurriculumAssignment {
            title: "Final Exam".into(),
            kind: "exam",
            notes: Some("Covers all modules".into()),
        });
    }
    if assignments.is_empty() {
        return None;
    }

    let topic = curriculum_topic_name(text, &modules);
    let subject = curriculum_subject_name(text, &topic);
    Some(StudyCurriculum {
        topic,
        subject,
        assignments,
    })
}

fn curriculum_topic_name(text: &str, modules: &[(u32, String, Vec<String>)]) -> String {
    let lower = text.to_ascii_lowercase();
    for marker in ["into topic ", "topic:", "topic "] {
        if let Some(idx) = lower.find(marker) {
            let after = &text[idx + marker.len()..];
            let name = after
                .lines()
                .next()
                .unwrap_or(after)
                .split(" and then")
                .next()
                .unwrap_or(after)
                .trim()
                .trim_end_matches(['.', ':'])
                .trim();
            if name.len() >= 3 && !name.to_ascii_lowercase().starts_with("module ") {
                return name
                    .chars()
                    .take(80)
                    .collect::<String>()
                    .trim()
                    .to_string();
            }
        }
    }
    if let Some((_, title, _)) = modules.first() {
        if title.to_ascii_lowercase().contains("introduction")
            || title.to_ascii_lowercase().starts_with("intro")
        {
            // Prefer "Intro to X" form when Module 1 is an introduction.
            let title_l = title.to_ascii_lowercase();
            if let Some(rest) = title_l
                .strip_prefix("introduction to ")
                .or_else(|| title_l.strip_prefix("intro to "))
            {
                let rest_orig = title
                    .split_once(" to ")
                    .or_else(|| title.split_once(" To "))
                    .map(|(_, r)| r.trim())
                    .unwrap_or(rest);
                return format!("Intro to {rest_orig}");
            }
        }
    }
    if lower.contains("cybersecurity") {
        return "Intro to Cybersecurity".into();
    }
    modules
        .first()
        .map(|(_, t, _)| t.clone())
        .unwrap_or_else(|| "Course".into())
}

fn curriculum_subject_name(text: &str, topic: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let topic_l = topic.to_ascii_lowercase();
    if lower.contains("cybersecurity") || topic_l.contains("cybersecurity") {
        return Some("Cybersecurity".into());
    }
    if let Some(rest) = topic_l
        .strip_prefix("intro to ")
        .or_else(|| topic_l.strip_prefix("introduction to "))
    {
        let name = topic
            .split_once(" to ")
            .or_else(|| topic.split_once(" To "))
            .map(|(_, r)| r.trim().to_string())
            .unwrap_or_else(|| {
                rest.split_whitespace()
                    .map(|w| {
                        let mut c = w.chars();
                        match c.next() {
                            Some(f) => format!("{}{}", f.to_ascii_uppercase(), c.as_str()),
                            None => String::new(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            });
        if name.len() >= 2 {
            return Some(name);
        }
    }
    None
}


fn parse_study_log(lower: &str) -> Option<String> {
    if !(has_word(lower, "study") || lower.contains("studied") || lower.contains("revision")) {
        return None;
    }
    let mut minutes: Option<i64> = None;
    if let Some(idx) = lower.find(" hour") {
        let prefix = &lower[..idx];
        if let Some(num) = prefix
            .rsplit(|c: char| !c.is_ascii_digit() && c != '.')
            .next()
            .and_then(|s| s.parse::<f64>().ok())
        {
            minutes = Some((num * 60.0).round() as i64);
        }
    }
    if minutes.is_none() {
        for marker in [" minutes", " minute", " mins", " min"] {
            if let Some(idx) = lower.find(marker) {
                let prefix = &lower[..idx];
                if let Some(n) = prefix
                    .rsplit(|c: char| !c.is_ascii_digit())
                    .next()
                    .and_then(|s| s.parse::<i64>().ok())
                {
                    minutes = Some(n);
                    break;
                }
            }
        }
    }
    let minutes = minutes.filter(|m| *m > 0)?;
    Some(json!({ "duration_minutes": minutes }).to_string())
}

fn parse_spark_fast(lower: &str, original: &str) -> Option<String> {
    if !looks_like_spark_save(lower) {
        return None;
    }
    let content = if let Some(i) = lower.find("spark:") {
        original[i + 6..].trim()
    } else if let Some(i) = lower.find("note to self") {
        original[i + 12..].trim().trim_start_matches(':').trim()
    } else if let Some(i) = lower.find("save this idea") {
        original[i + 14..].trim().trim_start_matches(':').trim()
    } else if let Some(i) = lower.find("save this thought") {
        original[i + 17..].trim().trim_start_matches(':').trim()
    } else {
        original.trim()
    };
    if content.len() < 2 {
        return None;
    }
    Some(json!({ "content": content, "tags": ["general_life"] }).to_string())
}

fn parse_fridge_add(lower: &str) -> Option<String> {
    let idx = if let Some(i) = lower.find(" to the fridge") {
        let before = &lower[..i];
        let start = before
            .rfind("add ")
            .map(|j| j + 4)
            .or_else(|| before.rfind("put ").map(|j| j + 4))?;
        start
    } else if let Some(i) = lower.find(" in the fridge") {
        let before = &lower[..i];
        let start = before
            .rfind("add ")
            .map(|j| j + 4)
            .or_else(|| before.rfind("put ").map(|j| j + 4))?;
        start
    } else {
        return None;
    };
    let end = lower[idx..]
        .find(" to the fridge")
        .or_else(|| lower[idx..].find(" in the fridge"))
        .unwrap_or(0);
    let name = lower[idx..idx + end]
        .trim()
        .trim_start_matches("a ")
        .trim_start_matches("an ")
        .trim_start_matches("some ")
        .trim();
    if name.len() < 2 {
        return None;
    }
    Some(
        json!({
            "action": "add",
            "name": name,
            "quantity": 1,
            "unit": "item",
        })
        .to_string(),
    )
}

/// High-precision in-app Document create: "make a document called X : body".
pub fn fast_docs_intent(text: &str) -> Option<(&'static str, String)> {
    if text.contains("~/") || text.contains("/Users/") || text.contains("/home/") {
        return None;
    }
    let title = extract_doc_title(text)?;
    let content = extract_doc_body(text, &title);
    if content.trim().is_empty() && wants_generated_doc_body(text) {
        return None;
    }
    Some((
        "docs.upsert",
        json!({ "title": title, "content": content }).to_string(),
    ))
}

fn wants_generated_doc_body(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("write about")
        || lower.contains("about the")
        || lower.contains("and write")
        || lower.contains("and add")
        || lower.contains("explaining")
        || lower.contains("covering")
        || lower.contains("make it about")
        || ((lower.contains("write a doc") || lower.contains("write a document"))
            && !lower.contains(':'))
}

pub(crate) fn extract_doc_title(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    const MARKERS: &[&str] = &[
        "document called ",
        "document named ",
        "document titled ",
        "doc called ",
        "doc named ",
        "doc titled ",
        "note called ",
        "note named ",
        "note titled ",
    ];
    for marker in MARKERS {
        if let Some(i) = lower.find(marker) {
            let rest = text[i + marker.len()..].trim_start();
            let token = rest
                .split(|c: char| {
                    c.is_whitespace() || matches!(c, ',' | '"' | '\'' | ':' | '`' | '!')
                })
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches(['!', ',']);
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

fn extract_doc_body(text: &str, title: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let tlow = title.to_ascii_lowercase();
    let start = lower.find(&tlow).map(|i| i + title.len()).unwrap_or(0);
    let after = text.get(start..).unwrap_or("");
    let mut search_from = 0;
    while let Some(rel) = after.get(search_from..).and_then(|s| s.find(':')) {
        let abs = search_from + rel;
        let after_colon = after.get(abs + 1..).unwrap_or("").trim_start();
        if after_colon.starts_with("//") {
            search_from = abs + 1;
            continue;
        }
        if after_colon.len() >= 12 {
            return after_colon.to_string();
        }
        search_from = abs + 1;
    }
    String::new()
}

pub fn wants_doc_format(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("format better")
        || lower.contains("better format")
        || lower.contains("reformat")
        || lower.contains("improve the format")
        || lower.contains("improve format")
        || lower.contains("cleaner format")
        || lower.contains("tidier format")
        || lower.contains("nicer format")
        || (lower.contains("format")
            && (lower.contains("cleaner") || lower.contains("tidier") || lower.contains("prettier")))
}

fn extract_format_doc_title(text: &str, ui_context: Option<&str>) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    if let Some(i) = lower.find("edit ") {
        let rest = text[i + 5..].trim_start();
        let token = rest
            .split(|c: char| c.is_whitespace() || matches!(c, ',' | '"' | '\'' | ':' | '`'))
            .next()
            .unwrap_or("")
            .trim();
        if token.len() >= 2 && !token.eq_ignore_ascii_case("the") {
            return Some(token.to_string());
        }
    }
    if let Some(title) = extract_doc_title(text) {
        return Some(title);
    }
    let ctx = ui_context.unwrap_or("");
    if let Some(i) = ctx.find("title=\"") {
        let rest = &ctx[i + 7..];
        if let Some(end) = rest.find('"') {
            let title = rest[..end].trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

pub fn fast_docs_format_intent(text: &str, ui_context: Option<&str>) -> Option<(&'static str, String)> {
    if !wants_doc_format(text) {
        return None;
    }
    if let Some((_, upsert_input)) = fast_docs_intent(text) {
        if let Ok(v) = serde_json::from_str::<Value>(&upsert_input) {
            let content = v.get("content").and_then(|c| c.as_str()).unwrap_or("");
            if content.len() >= 12 {
                return None;
            }
        }
    }
    let title = extract_format_doc_title(text, ui_context)?;
    Some(("docs.format", json!({ "id": title }).to_string()))
}



pub fn emit_workspace_focus(app: &AppHandle, tools: &[String], doc_output: Option<&str>) {
    let mut pages: Vec<String> = Vec::new();
    for name in tools {
        if let Some(page) = page_for_tool(name) {
            if !pages.iter().any(|p| p == page) {
                pages.push(page.to_string());
            }
        }
    }
    if pages.is_empty() {
        return;
    }
    let _ = app.emit(
        "workspace-focus",
        WorkspaceFocus {
            pages,
            focus_doc: doc_output.and_then(extract_doc_id),
        },
    );
}

pub fn page_for_tool(name: &str) -> Option<&'static str> {
    if name == "calendar.look"
        || name == "calendar.find_free_time"
        || name == "calendar.search_events"
        || name == "calendar.list_events"
        || name.starts_with("calendar.get_")
    {
        None
    } else if name.starts_with("calendar.")
        || name.starts_with("lifestyle.")
        || name.starts_with("dream.")
        || name.starts_with("work.")
    {
        Some("calendar")
    } else if name.starts_with("docs.") {
        Some("documents")
    } else if name.starts_with("todo.") {
        Some("todo")
    } else if name.starts_with("fitness.") {
        Some("fitness")
    } else if name.starts_with("money.") {
        Some("money")
    } else if name.starts_with("study.") {
        Some("study")
    } else if name.starts_with("socials.") {
        Some("socials")
    } else if name.contains("spark") {
        Some("spark")
    } else if name.starts_with("research.") {
        Some("chat")
    } else if name.starts_with("coder.") {
        Some("code")
    } else {
        None
    }
}

fn extract_doc_id(output: &str) -> Option<String> {
    let v: Value = serde_json::from_str(output).ok()?;
    v.get("id")
        .and_then(|id| id.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            v.get("document")
                .and_then(|d| d.get("id"))
                .and_then(|id| id.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
}

#[cfg(test)]
mod fast_calendar_tests {
    use super::fast_calendar_intent;

    #[test]
    fn whats_on_today_is_look() {
        let (tool, input) = fast_calendar_intent("what's on today?").unwrap();
        assert_eq!(tool, "calendar.look");
        assert!(input.contains("today"));
        assert!(input.contains("events"));
    }

    #[test]
    fn when_am_i_working_is_look() {
        let (tool, input) = fast_calendar_intent("when am I working?").unwrap();
        assert_eq!(tool, "calendar.look");
        assert!(input.contains("work"));
        assert!(input.contains("this_week"));
    }

    #[test]
    fn free_tomorrow_is_look() {
        let (tool, input) =
            fast_calendar_intent("when am I free tomorrow for 2 hours?").unwrap();
        assert_eq!(tool, "calendar.look");
        assert!(input.contains("free"));
        assert!(input.contains("120"));
    }

    #[test]
    fn dentist_clock_is_pin() {
        let (tool, input) = fast_calendar_intent("dentist tomorrow at 2pm").unwrap();
        assert_eq!(tool, "calendar.pin");
        assert!(input.contains("Dentist"));
        assert!(input.contains("tomorrow"));
    }

    #[test]
    fn book_climbing_week_not_fast() {
        assert!(fast_calendar_intent("book climbing this week").is_none());
        assert!(fast_calendar_intent("plan my week with study").is_none());
    }

    #[test]
    fn next_week_plans_is_look() {
        let (tool, input) = fast_calendar_intent("next week plans?").unwrap();
        assert_eq!(tool, "calendar.look");
        assert!(input.contains("next_week"), "{input}");
        assert!(input.contains("events"), "{input}");
    }

    #[test]
    fn whats_planned_next_week_is_look() {
        let (tool, input) = fast_calendar_intent("what's planned next week?").unwrap();
        assert_eq!(tool, "calendar.look");
        assert!(input.contains("next_week"), "{input}");
    }

    #[test]
    fn show_me_friday_is_look() {
        let (tool, input) = fast_calendar_intent("show me friday").unwrap();
        assert_eq!(tool, "calendar.look");
        assert!(input.contains("friday"), "{input}");
    }

    #[test]
    fn free_slots_next_week_not_today() {
        let (tool, input) =
            fast_calendar_intent("give me free slots next week not today").unwrap();
        assert_eq!(tool, "calendar.look");
        assert!(input.contains("\"focus\":\"free\""), "{input}");
        assert!(input.contains("next_week"), "{input}");
        assert!(!input.contains("\"when\":\"today\""), "{input}");
    }

    #[test]
    fn all_week_next_week_is_look() {
        let (tool, input) = fast_calendar_intent("give me all week next week").unwrap();
        assert_eq!(tool, "calendar.look");
        assert!(input.contains("next_week"), "{input}");
        assert!(input.contains("events"), "{input}");
    }

    #[test]
    fn free_slots_not_today_skips_today() {
        let (tool, input) = fast_calendar_intent("give me free slots not today").unwrap();
        assert_eq!(tool, "calendar.look");
        assert!(input.contains("not_today"), "{input}");
        assert!(input.contains("free"), "{input}");
    }
}

#[cfg(test)]
mod fast_docs_tests {
    use super::{fast_docs_format_intent, fast_docs_intent};

    #[test]
    fn create_named_doc_with_readme_body() {
        let msg = "make a document called bello.today and add make this readme.md a better format : This is a [Next.js](https://nextjs.org) project bootstrapped with create-next-app.";
        let (tool, input) = fast_docs_intent(msg).unwrap();
        assert_eq!(tool, "docs.upsert");
        assert!(input.contains("bello.today"), "{input}");
        assert!(input.contains("Next.js"), "{input}");
        assert!(!input.contains("~/"));
        assert!(fast_docs_format_intent(msg, None).is_none());
    }

    #[test]
    fn create_named_doc_without_body() {
        let (tool, input) = fast_docs_intent("make a document called hello-notes").unwrap();
        assert_eq!(tool, "docs.upsert");
        assert!(input.contains("hello-notes"), "{input}");
    }

    #[test]
    fn disk_path_is_not_docs() {
        assert!(fast_docs_intent("make a document called notes.md in ~/Desktop").is_none());
    }

    #[test]
    fn write_about_named_doc_skips_empty_fast_path() {
        assert!(fast_docs_intent(
            "write a doc called patatina a write about the jewellery brand thats made jewellery from recycled jewellary"
        )
        .is_none());
    }

    #[test]
    fn edit_bello_better_format_is_docs_format() {
        let msg = "edit bello.today document and make the format better please";
        let (tool, input) = fast_docs_format_intent(msg, None).unwrap();
        assert_eq!(tool, "docs.format");
        assert!(input.contains("bello.today"), "{input}");
    }

    #[test]
    fn format_uses_open_doc_from_ui_context() {
        let msg = "make the format better please";
        let ctx = r#"Page: documents. Open document id=abc title="bello.today" format=html."#;
        let (tool, input) = fast_docs_format_intent(msg, Some(ctx)).unwrap();
        assert_eq!(tool, "docs.format");
        assert!(input.contains("bello.today"), "{input}");
    }
}

#[cfg(test)]
mod life_tools_filter_tests {
    use super::{
        calendar_only_tool_filter, fast_calendar_intent, fast_life_look_intent,
        classify_life_dump, fast_life_write_intents, fast_life_write_intents_ctx, fast_stringed_plan,
        is_life_followup, is_multi_intent,
        is_trivial_chat, looks_like_spark_save, mixed_life_intent, page_for_tool,
        parse_study_curriculum, pick_spark_titles, pick_todo_titles, resolve_life_look_intent,
        selectors_for_turn, study_event_title, tool_prefixes_for_turn, turn_wants_tools, ChatMode,
    };
    use buddy_memory::HistoryMessage;

    #[test]
    fn pure_calendar_week_stays_calendar_only() {
        assert!(calendar_only_tool_filter("what's on this week?", None));
        assert!(!mixed_life_intent("what's on this week?"));
    }

    #[test]
    fn mixed_week_todos_is_not_calendar_only() {
        assert!(!calendar_only_tool_filter(
            "what do I need to get done this week?",
            None,
        ));
        assert!(mixed_life_intent(
            "what do i need to get done this week?"
        ));
        let prefixes = tool_prefixes_for_turn("what do I need to get done this week?", None);
        assert!(prefixes.iter().any(|p| *p == "todo."), "{prefixes:?}");
    }

    #[test]
    fn food_and_review_want_life_tools() {
        assert!(turn_wants_tools("what should I eat tonight?", None));
        assert!(tool_prefixes_for_turn("what should I eat tonight?", None)
            .iter()
            .any(|p| *p == "fitness."));
        assert!(tool_prefixes_for_turn("what do I weigh?", None)
            .iter()
            .any(|p| *p == "fitness."));
        assert!(tool_prefixes_for_turn("what did I spend this month?", None)
            .iter()
            .any(|p| *p == "money."));
        assert!(tool_prefixes_for_turn("show my sparks", None)
            .iter()
            .any(|p| *p == "list_sparks"));
        assert!(turn_wants_tools("sunday weekly review / social plan", None));
    }

    #[test]
    fn fitness_page_calendar_question_stays_calendar() {
        let prefixes = tool_prefixes_for_turn(
            "what's on this week?",
            Some("Page: fitness. Section Food."),
        );
        assert!(prefixes.iter().any(|p| *p == "calendar."), "{prefixes:?}");
        assert!(!prefixes.iter().any(|p| *p == "fitness."), "{prefixes:?}");
    }

    #[test]
    fn trivia_is_chat_even_on_documents_page() {
        let ctx = "Page: documents. No document open. Create or search with docs.upsert / docs.search.";
        assert!(!turn_wants_tools("whats the captial of the uk", Some(ctx)));
        assert!(!turn_wants_tools("tell me a short joke", Some(ctx)));
        assert!(tool_prefixes_for_turn("whats the captial of the uk", Some(ctx)).is_empty());
    }

    #[test]
    fn edit_doc_wants_docs_tools() {
        assert!(turn_wants_tools(
            "edit bello.today document and make the format better please",
            None,
        ));
        assert!(tool_prefixes_for_turn(
            "edit bello.today document and make the format better please",
            None,
        )
        .iter()
        .any(|p| *p == "docs."));
    }

    #[test]
    fn talk_mode_attaches_no_tools() {
        assert!(selectors_for_turn("dentist tomorrow at 2pm", None, ChatMode::Talk).is_empty());
        assert!(selectors_for_turn("I ate a burrito", None, ChatMode::Talk).is_empty());
    }

    #[test]
    fn trivia_attaches_no_tools() {
        let selectors = selectors_for_turn("whats the capital of the uk", None, ChatMode::Tool);
        assert!(selectors.is_empty(), "{selectors:?}");
    }

    #[test]
    fn list_workouts_is_fast_fitness_not_calendar() {
        assert!(fast_calendar_intent("list my logged workouts").is_none());
        assert!(fast_calendar_intent("what do i have in my workouts").is_none());
        let (tool, input) = fast_life_look_intent("list my logged workouts").unwrap();
        assert_eq!(tool, "fitness.look");
        assert!(input.contains("workouts"), "{input}");
        let selectors = selectors_for_turn("list my logged workouts", None, ChatMode::Tool);
        assert!(
            selectors.iter().all(|s| *s == "fitness."),
            "{selectors:?}"
        );
        assert!(fast_calendar_intent("do I have anything tomorrow?").is_some());
    }

    #[test]
    fn list_them_replays_last_workout_look() {
        assert!(is_life_followup("list them"));
        assert!(is_life_followup("answers : list them"));
        assert!(fast_life_look_intent("list them").is_none());
        let history = vec![HistoryMessage {
            role: "user".into(),
            content: "list my logged workouts".into(),
        }];
        let (tool, input) =
            resolve_life_look_intent("list them", None, &history, None).unwrap();
        assert_eq!(tool, "fitness.look");
        assert!(input.contains("workouts"), "{input}");
        let (tool, input) = resolve_life_look_intent(
            "list them",
            Some("Page: fitness. Section Workout."),
            &[],
            None,
        )
        .unwrap();
        assert_eq!(tool, "fitness.look");
        assert!(input.contains("workouts"), "{input}");
    }

    #[test]
    fn mixed_dump_maps_workspaces() {
        assert_eq!(page_for_tool("save_spark"), Some("spark"));
        assert_eq!(page_for_tool("docs.upsert"), Some("documents"));
        assert_eq!(page_for_tool("calendar.look"), None);
        assert_eq!(page_for_tool("calendar.organize"), Some("calendar"));
        assert_eq!(page_for_tool("fitness.log_food"), Some("fitness"));
        assert_eq!(page_for_tool("money.log"), Some("money"));
    }

    #[test]
    fn study_sheet_is_docs_not_study_sessions() {
        assert!(fast_calendar_intent("whats in my study sheet").is_none());
        let (tool, input) = fast_life_look_intent("whats in my study sheet").unwrap();
        assert_eq!(tool, "docs.search");
        assert!(input.contains("study sheet"), "{input}");
        let history = vec![HistoryMessage {
            role: "user".into(),
            content: "whats in my study sheet".into(),
        }];
        let (tool, _) = resolve_life_look_intent("list them", None, &history, None).unwrap();
        assert_eq!(tool, "docs.search");
    }

    #[test]
    fn spend_dump_is_fast_money_log_not_list() {
        assert!(fast_life_look_intent("i spent £10 at the market today").is_none());
        let jobs = fast_life_write_intents("i spent £10 at the market today");
        assert_eq!(jobs.len(), 1, "{jobs:?}");
        assert_eq!(jobs[0].0, "money.log");
        assert!(jobs[0].1.contains("10"), "{}", jobs[0].1);
        assert!(jobs[0].1.contains("market"), "{}", jobs[0].1);
        assert!(fast_calendar_intent("i spent £10 at the market today").is_none());
        let reads = fast_life_write_intents("what did i spend this month?");
        assert!(reads.is_empty(), "{reads:?}");
        let (tool, _) = fast_life_look_intent("what did i spend this month?").unwrap();
        assert_eq!(tool, "money.list");

        let rent = fast_life_write_intents("rent was £497.5");
        assert_eq!(rent.len(), 1, "{rent:?}");
        assert_eq!(rent[0].0, "money.log");
        assert!(rent[0].1.contains("497.5"), "{}", rent[0].1);
        assert!(rent[0].1.contains("rent"), "{}", rent[0].1);
        assert!(rent[0].1.contains("expense"), "{}", rent[0].1);
        assert!(fast_life_write_intents("holiday pot £200").iter().all(|(t, _)| *t != "money.log"));
    }

    #[test]
    fn savings_pots_parse_as_money_pot() {
        let set = fast_life_write_intents("holiday pot £200, emergency £500");
        assert_eq!(set.len(), 2, "{set:?}");
        assert!(set.iter().all(|(t, _)| *t == "money.pot"), "{set:?}");
        assert!(set[0].1.contains("holiday"), "{}", set[0].1);
        assert!(set[0].1.contains("200"), "{}", set[0].1);
        assert!(set[0].1.contains("set"), "{}", set[0].1);
        assert!(set[1].1.contains("emergency"), "{}", set[1].1);
        assert!(set[1].1.contains("500"), "{}", set[1].1);

        let add = fast_life_write_intents("put £50 in the holiday pot");
        assert_eq!(add.len(), 1, "{add:?}");
        assert_eq!(add[0].0, "money.pot");
        assert!(add[0].1.contains("holiday"), "{}", add[0].1);
        assert!(add[0].1.contains("50"), "{}", add[0].1);
        assert!(add[0].1.contains("add"), "{}", add[0].1);

        let dump = classify_life_dump(
            "pots: holiday £200 emergency £1500",
            Some("Page: money."),
        );
        assert!(dump.money && dump.blocks_complete());
        assert!(fast_life_write_intents("rent was £497.5")
            .iter()
            .all(|(t, _)| *t == "money.log"));
    }

    #[test]
    fn dump_classifier_blocks_complete_for_logs() {
        let rent = classify_life_dump("rent was £497.5", None);
        assert!(rent.money && rent.blocks_complete());
        assert!(!rent.food && !rent.curriculum);

        let food = classify_life_dump("i ate a burrito", None);
        assert!(food.food && food.blocks_complete());

        let fitness_had = classify_life_dump(
            "i had a burrito",
            Some("Page: fitness. Section Food."),
        );
        assert!(fitness_had.food && fitness_had.blocks_complete());
        let jobs = fast_life_write_intents_ctx(
            "i had a burrito",
            Some("Page: fitness. Section Food."),
        );
        assert_eq!(jobs[0].0, "fitness.log_food");

        let money_page = classify_life_dump("£12.50", Some("Page: money."));
        assert!(money_page.money && money_page.blocks_complete());

        let question = classify_life_dump("what did i spend this month?", None);
        assert!(!question.blocks_complete());
        let meeting = classify_life_dump(
            "i had a meeting",
            Some("Page: fitness. Section Food."),
        );
        assert!(!meeting.food);

        let syllabus = classify_life_dump(
            "Intro to Cybersecurity\nModule 1: Basics\nAssignment: Module Quiz\nFinal Exam",
            Some("Page: study."),
        );
        assert!(syllabus.curriculum && syllabus.blocks_complete());

        let selectors = selectors_for_turn("rent was £497.5", None, ChatMode::Tool);
        assert!(selectors.iter().all(|s| *s == "money."), "{selectors:?}");
    }

    #[test]
    fn ate_and_todo_and_study_are_fast_writes() {
        let food = fast_life_write_intents("i ate a burrito");
        assert_eq!(food[0].0, "fitness.log_food");
        assert!(food[0].1.contains("burrito"), "{}", food[0].1);
        assert!(food[0].1.contains("calories"), "{}", food[0].1);

        let multi = fast_life_write_intents(
            "in food and calories i had a bowl of honey flakes cereal with oat milk, then a blt with mortedella not bacon and an apple",
        );
        assert_eq!(multi.len(), 3, "{multi:?}");
        assert!(multi.iter().all(|(t, _)| *t == "fitness.log_food"), "{multi:?}");
        let joined = multi.iter().map(|(_, i)| i.as_str()).collect::<Vec<_>>().join("\n");
        assert!(joined.contains("honey flakes cereal"), "{joined}");
        assert!(joined.contains("blt") || joined.contains("mort"), "{joined}");
        assert!(joined.contains("apple"), "{joined}");

        let todo = fast_life_write_intents("add todo buy milk");
        assert_eq!(todo[0].0, "todo.add");
        assert!(todo[0].1.contains("buy milk"), "{}", todo[0].1);

        let study = fast_life_write_intents("i studied for 45 minutes");
        assert_eq!(study[0].0, "study.log_session");
        assert!(study[0].1.contains("45"), "{}", study[0].1);

        let both = fast_life_write_intents("i ate a burrito and spent £10 at the market");
        assert!(both.iter().any(|(t, _)| *t == "fitness.log_food"), "{both:?}");
        assert!(both.iter().any(|(t, _)| *t == "money.log"), "{both:?}");
    }

    #[test]
    fn research_and_socials_fast_looks() {
        let (tool, _) = fast_life_look_intent("list my research").unwrap();
        assert_eq!(tool, "research.list");
        let (tool, input) = fast_life_look_intent("show my social drafts").unwrap();
        assert_eq!(tool, "socials.look");
        assert!(input.contains("drafts"), "{input}");
    }

    #[test]
    fn stringed_spark_todos_slot_is_multi_intent() {
        let msg = "find a slot for a spark and two easy to dos tomorrow";
        assert!(is_multi_intent(msg, None));
        assert!(fast_calendar_intent(msg).is_none());
        assert!(fast_life_look_intent(msg).is_none());
        let plan = fast_stringed_plan(msg, None).expect("stringed plan");
        assert!(
            plan.iter().any(|(t, _)| *t == "calendar.look"),
            "{plan:?}"
        );
        assert!(
            plan.iter().any(|(t, _)| *t == "list_sparks"),
            "{plan:?}"
        );
        assert_eq!(
            plan.iter().filter(|(t, _)| *t == "todo.list").count(),
            1,
            "{plan:?}"
        );
        assert!(
            plan.iter().all(|(t, _)| *t != "todo.add"),
            "must not invent Easy task N: {plan:?}"
        );
        let prefixes = tool_prefixes_for_turn(msg, None);
        assert!(prefixes.iter().any(|p| *p == "calendar."), "{prefixes:?}");
        assert!(prefixes.iter().any(|p| *p == "todo."), "{prefixes:?}");
        assert!(prefixes.iter().any(|p| *p == "save_spark"), "{prefixes:?}");
        let selectors = selectors_for_turn(msg, None, ChatMode::Tool);
        assert!(selectors.iter().any(|s| *s == "calendar."), "{selectors:?}");
        assert!(selectors.iter().any(|s| *s == "todo."), "{selectors:?}");
        assert!(selectors.iter().any(|s| *s == "save_spark"), "{selectors:?}");
        assert!(
            !selectors.iter().any(|s| *s == "fitness."),
            "should not dump full kit: {selectors:?}"
        );
    }

    #[test]
    fn study_event_tomorrow_wants_calendar_and_study() {
        let msg = "for my study event tomorrow can you add the topic and the first two assignments";
        assert!(is_multi_intent(msg, None));
        let prefixes = tool_prefixes_for_turn(msg, None);
        assert!(prefixes.iter().any(|p| *p == "calendar."), "{prefixes:?}");
        assert!(prefixes.iter().any(|p| *p == "study."), "{prefixes:?}");
        assert!(fast_calendar_intent(msg).is_none());
        let ctx = r#"Page: study. Active subject "Cybersecurity" (id=844b2d5f-ca2d-4d93-aab3-9c03b6c8f8cd)."#;
        let plan = fast_stringed_plan(msg, Some(ctx)).expect("study plan");
        assert!(plan.iter().any(|(t, _)| *t == "calendar.look"), "{plan:?}");
        assert!(
            plan.iter()
                .any(|(t, i)| *t == "study.look" && i.contains("topics")),
            "{plan:?}"
        );
        assert!(
            plan.iter()
                .any(|(t, i)| *t == "study.look" && i.contains("assignments")),
            "{plan:?}"
        );
        assert!(
            plan.iter().all(|(t, _)| *t != "study.upsert_assignment"),
            "assignments come from study.look, not invented titles: {plan:?}"
        );
    }

    #[test]
    fn stringed_picks_real_spark_and_easy_todos() {
        let sparks = r#"[
            {"content":"try cold brew on the balcony","status":"active","updated_at":20},
            {"content":"old archived idea","status":"archived","updated_at":99}
        ]"#;
        let titles = pick_spark_titles(sparks, 1);
        assert_eq!(titles, vec!["try cold brew on the balcony"]);

        let todos = r#"[
            {"title":"Tax return","priority":"high","status":"not_started","deadline":"2026-08-11"},
            {"title":"Water plants","priority":"low","status":"not_started","deadline":"2026-08-12","overdue":false},
            {"title":"Wipe counters","priority":"low","status":"not_started","deadline":"2026-08-10","overdue":true},
            {"title":"Done already","priority":"low","status":"completed","deadline":"2026-08-01"}
        ]"#;
        let picked = pick_todo_titles(todos, 2, true);
        assert_eq!(picked, vec!["Wipe counters".to_string(), "Water plants".to_string()]);

        let look = r#"{"focus":"free","events":[{"title":"Study"},{"title":"Gym"}]}"#;
        assert_eq!(study_event_title(look).as_deref(), Some("Study"));
    }

    #[test]
    fn parses_cybersecurity_module_syllabus() {
        let text = r#"add this to my study planner into topic intro to Cybersecurity and then the assignments is :
Module 1: Introduction to Cybersecurity
Topics: What is cybersecurity?
The CIA triad (Confidentiality, Integrity, Availability).
Assignment: Module Quiz.

Module 2: Attacks, Concepts, and Techniques
Topics: Common cyber threats (malware, phishing, social engineering).
Assignment: Module Quiz.

Module 5: Will Your Future Be in Cybersecurity?
Assignment: Module Quiz.

Final Assessment
Assignment: Final Exam (covers all modules).
WORK
on Tue 11 Aug, 8:45 AM–4:45 PM"#;
        let plan = parse_study_curriculum(text).expect("curriculum");
        assert_eq!(plan.topic.to_ascii_lowercase(), "intro to cybersecurity");
        assert_eq!(plan.subject.as_deref(), Some("Cybersecurity"));
        assert_eq!(plan.assignments.len(), 4);
        assert_eq!(plan.assignments[0].title, "Module 1 Quiz");
        assert_eq!(plan.assignments[0].kind, "assignment");
        assert_eq!(plan.assignments[1].title, "Module 2 Quiz");
        assert_eq!(plan.assignments[2].title, "Module 5 Quiz");
        assert_eq!(plan.assignments[3].title, "Final Exam");
        assert_eq!(plan.assignments[3].kind, "exam");
        assert!(plan.assignments[0]
            .notes
            .as_deref()
            .unwrap_or("")
            .contains("CIA triad"));
    }

    #[test]
    fn trivial_chat_skips_tools_messy_dump_does_not() {
        assert!(is_trivial_chat("how are you?", None));
        assert!(is_trivial_chat("whats the capital of the uk", None));
        // Extractable todos still look like chitchat to this heuristic.
        // orchestrator.route() takes them first, before talk policy.
        assert!(is_trivial_chat("remind me to call the dentist", None));
        assert!(!is_trivial_chat(
            "Tomorrow I have the dentist at 10, need to buy milk after work, spent £25 on dinner, and I had an idea for a climbing tracker.",
            None,
        ));
    }

    #[test]
    fn dump_holdout_1_is_multi_intent_not_trivial() {
        let msg = "had eggs for breakfast, dentist tomorrow at 2pm, spark: local climbing beta app — deep dive that into a project doc and block two 90 min build slots this week, spent £12 on lunch";
        assert!(!is_trivial_chat(msg, None));
        assert!(is_multi_intent(msg, None));
        let prefixes = tool_prefixes_for_turn(msg, None);
        assert!(
            prefixes.iter().any(|p| p.starts_with("calendar.")),
            "{prefixes:?}"
        );
        assert!(
            prefixes
                .iter()
                .any(|p| *p == "save_spark" || *p == "list_sparks" || *p == "update_spark"),
            "{prefixes:?}"
        );
        assert!(prefixes.iter().any(|p| *p == "money."), "{prefixes:?}");
        let selectors = selectors_for_turn(msg, None, ChatMode::Tool);
        assert!(!selectors.is_empty(), "{selectors:?}");
        assert!(selectors_for_turn(msg, None, ChatMode::Talk).is_empty());
    }

    #[test]
    fn dump_holdout_2_and_3_select_expected_families() {
        let ate = "I ate a burrito and logged a 6a at the wall";
        assert!(!is_trivial_chat(ate, None));
        let fitness = tool_prefixes_for_turn(ate, None);
        assert!(fitness.iter().any(|p| *p == "fitness."), "{fitness:?}");

        let spark = "note to self: try meal prep on Sundays";
        assert!(looks_like_spark_save(&spark.to_ascii_lowercase()) || !is_trivial_chat(spark, None));
        let prefixes = tool_prefixes_for_turn(spark, None);
        assert!(
            prefixes
                .iter()
                .any(|p| *p == "save_spark" || *p == "list_sparks"),
            "{prefixes:?}"
        );
    }

    #[test]
    fn llama_talk_never_receives_tools() {
        assert!(selectors_for_turn("how are you?", None, ChatMode::Talk).is_empty());
        assert!(selectors_for_turn(
            "had eggs for breakfast, dentist tomorrow at 2pm",
            None,
            ChatMode::Talk
        )
        .is_empty());
    }
}

#[cfg(test)]
mod gateway_loop_tests {
    use super::{
        inject_idempotency_key, looks_like_goal_intake, selectors_for_turn,
        should_follow_up_complete, ChatMode,
    };

    #[test]
    fn day_dump_independent_writes_stay_one_model_call() {
        let tools = vec![
            "fitness.log_food".into(),
            "money.log".into(),
            "save_spark".into(),
        ];
        assert!(!should_follow_up_complete(&tools, 1, 2, false));
    }

    #[test]
    fn look_then_decide_allows_one_follow_up() {
        let tools = vec!["calendar.look".into(), "goal.look".into()];
        assert!(should_follow_up_complete(&tools, 1, 2, false));
        assert!(!should_follow_up_complete(&tools, 2, 2, false));
        assert!(!should_follow_up_complete(&tools, 1, 2, true));
    }

    #[test]
    fn v6_goal_sentence_attaches_only_goal_tools() {
        let sel = selectors_for_turn(
            "I want to climb V6 by the end of November",
            None,
            ChatMode::Tool,
        );
        assert_eq!(sel, vec!["goal.intake", "goal.look"]);
        assert!(looks_like_goal_intake(
            "i want to climb v6 by end of november"
        ));
        assert!(!looks_like_goal_intake("i climbed a 6a today"));
    }

    #[test]
    fn goal_intake_gets_turn_index_idempotency_key() {
        let out = inject_idempotency_key(
            "goal.intake",
            r#"{"title":"Climb V6","deadline":"2026-11-30"}"#,
            "turn-a:0",
        );
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["idempotency_key"], "turn-a:0");
        assert_eq!(v["title"], "Climb V6");
        assert_eq!(
            inject_idempotency_key("calendar.look", "{}", "turn-a:0"),
            "{}"
        );
    }
}

