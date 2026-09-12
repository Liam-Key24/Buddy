//! One turn: Memory → canonical actions → clarify → Core → one reply.
//! Messy language goes to Qwen (`/v1/complete`). Llama is a hidden chat cache.

use std::sync::Arc;
use std::time::{Duration, Instant};

use buddy_clarification::{
    clarify, is_cancel_phrase, is_confirm_phrase, is_soft_constraint_phrase,
    looks_like_proposal, merge_field_value, merge_free_text_into_tool_input, set_apply_flag,
    AskChoice, ClarifyResult, ClarificationConfig, JobPhase, MissingField, PendingClarification,
    PreferenceLookup,
};
use buddy_core::{merge_session_into_input, AskKind, Route, RouteKind, SessionContext};
use buddy_memory::{HistoryMessage, MemoryContext, MemoryEvent};
use buddy_personality::{
    phrase_clarification, phrase_tool_result, style_response, ClarificationAsk,
    PersonalityProfile,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, info, instrument, warn};

use buddy_database::{DbError, SPARK_NUDGE_COOLDOWN_MS, SPARK_STALE_AGE_MS};

use crate::calendar_look;
use crate::calendar_proposal::{self, clear_proposal};
use crate::memory_api;
use crate::memory_extraction::BrainMemoryContext;
use crate::native_loop::{self, load_transcript, NativeOutcome};
use crate::run_control::{RunGuard, RunScope};
use crate::services::{talk_recovery, ProcessManager, TalkRecovery};
use crate::state::AppState;
use crate::turn_controller::ModelLane;
use crate::turn_trace::{self, TurnPath, TurnTrace};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScratchStep {
    pub tool: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct AgentTurn {
    pub goal: String,
    pub scratchpad: Vec<ScratchStep>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RespondRequest {
    message: String,
    history: Vec<HistoryMessage>,
    memory: BrainMemoryContext,
    tool_name: Option<String>,
    tool_result: Option<String>,
}

#[instrument(skip(state, app), fields(conversation_id = %conversation_id))]
pub async fn send_message(
    app: AppHandle,
    state: &AppState,
    conversation_id: String,
    text: String,
    ui_context: Option<String>,
) -> Result<(), String> {
    info!(text = %text, "user request");
    emit_trace(&app, "planning", "Reading context");

    let started = Instant::now();
    let mut trace = TurnTrace::new(&conversation_id);

    let _scope = RunScope::start(&state.runs, &conversation_id);
    let run = &_scope.guard;

    let ctx = state.memory.ctx(&conversation_id);
    let merged = state.memory.get_context(&conversation_id, &text).await;
    let history = merged.conversation_messages.clone();
    state
        .memory
        .maybe_auto_handover(state, &conversation_id, &merged, &history)
        .await;

    state
        .db
        .add_message(&conversation_id, "user", &text)
        .map_err(|e| e.to_string())?;
    let _ = state.memory.store_event(
        &ctx,
        MemoryEvent::MessageAdded {
            role: "user".into(),
            content: text.clone(),
        },
    );

    if state
        .db
        .get_messages(&conversation_id)
        .map(|m| m.len())
        .unwrap_or(0)
        == 1
    {
        let title: String = text.chars().take(40).collect();
        let _ = state.db.update_conversation_title(&conversation_id, &title);
    }

    let mut memory = state.memory.brain_payload(&merged);
    state
        .memory
        .enrich_with_pending(&mut memory, &conversation_id);
    let personality = load_personality(state);
    let client = reqwest::Client::new();

    // Conflict override: "allow" / "force" retries the pending create/update with force:true.
    if let Some((content, tool_name)) = try_force_confirm_pending(
        &app,
        state,
        &client,
        &ctx,
        &conversation_id,
        &text,
        &history,
        &memory,
        &personality,
    )
    .await?
    {
        let assistant_metadata = serde_json::json!({
            "intent": "tool_use",
            "tool": tool_name,
            "respond_mode": "passthrough",
            "force_confirm": true,
        })
        .to_string();
        state
            .db
            .add_message_with_metadata(
                &conversation_id,
                "assistant",
                &content,
                Some(&assistant_metadata),
            )
            .map_err(|e| e.to_string())?;
        let _ = state.memory.store_event(
            &ctx,
            MemoryEvent::MessageAdded {
                role: "assistant".into(),
                content: content.clone(),
            },
        );
        trace.tool_steps = 1;
        seal_trace(&app, state, &mut trace, TurnPath::ForceConfirm, started);
        let _ = app.emit("chat-done", ());
        return Ok(());
    }

    // Confirm an open organize proposal without a model round.
    if is_confirm_phrase(&text) {
        if calendar_proposal::load_proposal(state, Some(&conversation_id)).is_some()
            || calendar_proposal::load_proposal(state, None).is_some()
        {
            match calendar_proposal::commit_stored_proposal(
                &app,
                state,
                Some(&conversation_id),
                Some(&personality),
            )
            .await
            {
                Ok(content) => {
                    native_loop::emit_workspace_focus(
                        &app,
                        &["calendar.organize".to_string()],
                        None,
                    );
                    state
                        .db
                        .add_message_with_metadata(
                            &conversation_id,
                            "assistant",
                            &content,
                            Some(
                                &json!({
                                    "intent": "tool_use",
                                    "tool": "calendar.organize",
                                    "mode": "commit",
                                })
                                .to_string(),
                            ),
                        )
                        .map_err(|e| e.to_string())?;
                    let _ = state.memory.store_event(
                        &ctx,
                        MemoryEvent::MessageAdded {
                            role: "assistant".into(),
                            content: content.clone(),
                        },
                    );
                    trace.tool_steps = 1;
                    seal_trace(&app, state, &mut trace, TurnPath::ProposalCommit, started);
                    let _ = app.emit("chat-done", ());
                    return Ok(());
                }
                Err(e) => {
                    info!(error = %e, "proposal commit skipped");
                }
            }
        }
    }

    if is_cancel_phrase(&text) {
        if calendar_proposal::load_proposal(state, Some(&conversation_id)).is_some() {
            clear_proposal(state, &app, Some(&conversation_id));
            state.memory.clear_pending_clarification(&conversation_id);
            state.memory.clear_agent_turn(&conversation_id);
            let content = style_response(&personality, "Okay — dismissed the proposed week.");
            let _ = app.emit("chat-chunk", &content);
            state
                .db
                .add_message(&conversation_id, "assistant", &content)
                .map_err(|e| e.to_string())?;
            seal_trace(&app, state, &mut trace, TurnPath::ProposalCancel, started);
            let _ = app.emit("chat-done", ());
            return Ok(());
        }
    }

    let work = state.memory.get_work_item(&conversation_id);
    let pending_open = work
        .as_ref()
        .and_then(|item| item.pending_clarification.as_ref())
        .is_some();
    let has_native = load_transcript(state, &conversation_id).is_some();

    // One front door: canonical syntax / plugin extract → Core. Messy NL → Qwen.
    if !pending_open {
        if let Some(content) = try_canonical_route(
            &app,
            state,
            &ctx,
            &conversation_id,
            &text,
            &personality,
            &mut trace,
        )
        .await?
        {
            let path = match state.plugins.route_kind(&text) {
                RouteKind::Extract => TurnPath::Extract,
                _ => TurnPath::Canonical,
            };
            seal_trace(&app, state, &mut trace, path, started);
            persist_assistant_turn(
                &app,
                state,
                &ctx,
                &conversation_id,
                &content,
                &json!({"intent":"tool_use","canonical":true}).to_string(),
            )?;
            return Ok(());
        }
        if let Some(content) =
            calendar_look::try_resolve(&app, state, &conversation_id, &personality, &text)
        {
            seal_trace(&app, state, &mut trace, TurnPath::LastLook, started);
            persist_assistant_turn(
                &app,
                state,
                &ctx,
                &conversation_id,
                &content,
                &json!({"intent":"last_look"}).to_string(),
            )?;
            return Ok(());
        }
    }

    // Resume native transcript (no re-classify).
    if has_native {
        wake_mlx(&app, state).await;
        match native_loop::run_native_turn(
            &app,
            state,
            &client,
            &ctx,
            &conversation_id,
            &text,
            ui_context.as_deref(),
            &history,
            &memory,
            &personality,
            true,
            run,
            native_loop::ChatMode::Tool,
            &mut trace,
        )
        .await?
        {
            NativeOutcome::Stopped(content) => {
                seal_trace(&app, state, &mut trace, TurnPath::ResumeNative, started);
                persist_assistant_turn(
                    &app,
                    state,
                    &ctx,
                    &conversation_id,
                    &content,
                    &json!({"intent":"stopped","resume":true}).to_string(),
                )?;
                return Ok(());
            }
            NativeOutcome::Done(content) | NativeOutcome::Paused(content) => {
                state
                    .db
                    .add_message_with_metadata(
                        &conversation_id,
                        "assistant",
                        &content,
                        Some(&json!({"intent":"native","resume":true}).to_string()),
                    )
                    .map_err(|e| e.to_string())?;
                let _ = state.memory.store_event(
                    &ctx,
                    MemoryEvent::MessageAdded {
                        role: "assistant".into(),
                        content: content.clone(),
                    },
                );
                seal_trace(&app, state, &mut trace, TurnPath::ResumeNative, started);
                let _ = app.emit("chat-done", ());
                return Ok(());
            }
            NativeOutcome::Fallback(_) => {}
        }
    }

    // Open-job continuity: merge free-text / confirm into the same job before re-classify.
    if let Some(content) = try_continue_open_job(
        &app,
        state,
        &client,
        &ctx,
        &conversation_id,
        &text,
        &history,
        &memory,
        &personality,
    )
    .await?
    {
        state
            .db
            .add_message_with_metadata(
                &conversation_id,
                "assistant",
                &content,
                Some(
                    &json!({
                        "intent": "open_job",
                        "goal": state
                            .memory
                            .get_pending_clarification(&conversation_id)
                            .and_then(|p| p.agent_goal)
                            .unwrap_or_default(),
                    })
                    .to_string(),
                ),
            )
            .map_err(|e| e.to_string())?;
        let _ = state.memory.store_event(
            &ctx,
            MemoryEvent::MessageAdded {
                role: "assistant".into(),
                content: content.clone(),
            },
        );
        seal_trace(&app, state, &mut trace, TurnPath::OpenJob, started);
        let _ = app.emit("chat-done", ());
        return Ok(());
    }

    let trivial = native_loop::is_trivial_chat(&text, ui_context.as_deref());
    let qwen_resident = app
        .try_state::<Arc<ProcessManager>>()
        .is_some_and(|pm| pm.tool_model_loaded(state));
    let lane = ModelLane::select(trivial, qwen_resident);
    let chat_mode = lane.native_mode();

    // Llama is only a latency cache for short chitchat when Qwen is not already loaded.
    if lane == ModelLane::LlamaTalk {
        match stream_talk_reply(
            &app,
            state,
            &client,
            &text,
            &history,
            &memory,
            &personality,
            run,
            &mut trace,
        )
        .await
        {
            Ok(content) => {
                seal_trace(&app, state, &mut trace, TurnPath::TalkLlama, started);
                persist_assistant_turn(
                    &app,
                    state,
                    &ctx,
                    &conversation_id,
                    &content,
                    &json!({"intent":"chat"}).to_string(),
                )?;
                return Ok(());
            }
            Err(err) => {
                warn!(error = %err, "chat stream failed — falling through to Qwen");
            }
        }
    }

    wake_mlx(&app, state).await;
    match native_loop::run_native_turn(
        &app,
        state,
        &client,
        &ctx,
        &conversation_id,
        &text,
        ui_context.as_deref(),
        &history,
        &memory,
        &personality,
        false,
        run,
        chat_mode,
        &mut trace,
    )
    .await?
    {
        NativeOutcome::Stopped(content) => {
            seal_trace(&app, state, &mut trace, TurnPath::CompleteQwen, started);
            persist_assistant_turn(
                &app,
                state,
                &ctx,
                &conversation_id,
                &content,
                &json!({"intent":"stopped"}).to_string(),
            )?;
            return Ok(());
        }
        NativeOutcome::Done(content) | NativeOutcome::Paused(content) => {
            state
                .db
                .add_message_with_metadata(
                    &conversation_id,
                    "assistant",
                    &content,
                    Some(&json!({"intent":"native"}).to_string()),
                )
                .map_err(|e| e.to_string())?;
            let _ = state.memory.store_event(
                &ctx,
                MemoryEvent::MessageAdded {
                    role: "assistant".into(),
                    content: content.clone(),
                },
            );
            seal_trace(&app, state, &mut trace, TurnPath::CompleteQwen, started);
            let _ = app.emit("chat-done", ());
            return Ok(());
        }
        NativeOutcome::Fallback(err) => {
            info!(error = %err, "native loop failed");
            let content = style_response(
                &personality,
                "I couldn't reach the local model. Try again in a moment.",
            );
            seal_trace(&app, state, &mut trace, TurnPath::ModelError, started);
            persist_assistant_turn(
                &app,
                state,
                &ctx,
                &conversation_id,
                &content,
                &json!({"intent":"error","native":true}).to_string(),
            )?;
            return Ok(());
        }
    }
}

pub(crate) async fn wake_mlx(app: &AppHandle, state: &AppState) {
    let Some(pm) = app.try_state::<Arc<ProcessManager>>() else {
        return;
    };
    emit_trace(app, "planning", "Starting local model…");
    if let Err(e) = pm.ensure_mlx(state).await {
        warn!(error = %e, "on-demand mlx start failed");
    }
}

async fn wake_chat_mlx(app: &AppHandle, state: &AppState) {
    let Some(pm) = app.try_state::<Arc<ProcessManager>>() else {
        return;
    };
    emit_trace(app, "planning", "Starting Llama chat…");
    if let Err(e) = pm.ensure_chat_mlx(state).await {
        warn!(error = %e, "on-demand llama chat start failed");
    }
}

fn compact_tool_summary(tool: &str, output: &str) -> String {
    let trimmed = output.trim();
    if trimmed.len() <= 800 {
        return format!("{tool}: {trimmed}");
    }
    format!("{tool}: {}…", &trimmed[..800])
}

fn save_agent_turn(state: &AppState, conversation_id: &str, turn: &AgentTurn) {
    if let Ok(raw) = serde_json::to_string(turn) {
        state.memory.set_agent_turn(conversation_id, &raw);
    }
}

fn load_agent_turn(state: &AppState, conversation_id: &str, fallback_goal: &str) -> AgentTurn {
    if let Some(raw) = state.memory.get_agent_turn(conversation_id) {
        if let Ok(t) = serde_json::from_str::<AgentTurn>(&raw) {
            return t;
        }
    }
    AgentTurn {
        goal: fallback_goal.to_string(),
        scratchpad: vec![],
    }
}

/// Continue an open job (needs_input / awaiting_confirm) without re-classifying.
#[allow(clippy::too_many_arguments)]
async fn try_continue_open_job(
    app: &AppHandle,
    state: &AppState,
    _client: &reqwest::Client,
    ctx: &MemoryContext,
    conversation_id: &str,
    text: &str,
    _history: &[HistoryMessage],
    _memory: &BrainMemoryContext,
    personality: &PersonalityProfile,
) -> Result<Option<String>, String> {
    let Some(pending) = state.memory.get_pending_clarification(conversation_id) else {
        return Ok(None);
    };
    // Force-conflict sentinel uses missing "force" — leave to try_force_confirm_pending.
    if pending_is_force(&pending) {
        return Ok(None);
    }
    if pending.follow_up {
        return Ok(None);
    }

    if is_cancel_phrase(text)
        && matches!(
            pending.phase,
            JobPhase::NeedsInput | JobPhase::AwaitingConfirm
        )
    {
        state.memory.clear_pending_clarification(conversation_id);
        state.memory.clear_agent_turn(conversation_id);
        let content = style_response(personality, "Okay — cancelled.");
        let _ = app.emit("chat-chunk", &content);
        return Ok(Some(content));
    }

    let goal = if pending.goal().is_empty() {
        text.to_string()
    } else {
        pending.goal().to_string()
    };
    let mut turn = AgentTurn {
        goal: goal.clone(),
        scratchpad: pending
            .agent_scratchpad
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default(),
    };

    match pending.phase {
        JobPhase::NeedsInput => {
            let merged = merge_free_text_into_tool_input(&pending.tool_input, text);
            // Also try merging primary missing field when it's a choice label match.
            let merged = if let Some(field) = pending.primary_missing() {
                if field.ask_kind == buddy_core::AskKind::Choice {
                    if let Some(choice) = field.choices.iter().find(|c| {
                        c.label.eq_ignore_ascii_case(text.trim())
                            || c.id.eq_ignore_ascii_case(text.trim())
                            || c.value == text.trim()
                    }) {
                        merge_field_value(&merged, &field.name, &choice.value)
                    } else {
                        merged
                    }
                } else if field.name == "title"
                    && !text.trim().is_empty()
                    && buddy_clarification::parse_duration_minutes(text).is_none()
                {
                    merge_field_value(&merged, "title", &format!("\"{}\"", text.trim().replace('"', "")))
                } else {
                    merged
                }
            } else {
                merged
            };

            let content = dispatch_tool(
                app,
                state,
                ctx,
                conversation_id,
                &goal,
                personality,
                &pending.tool,
                &merged,
                &mut turn,
            )
            .await?;
            Ok(Some(content))
        }
        JobPhase::AwaitingConfirm => {
            if is_confirm_phrase(text) {
                let applied = set_apply_flag(&pending.tool_input, true);
                let content = dispatch_tool(
                    app,
                    state,
                    ctx,
                    conversation_id,
                    &goal,
                    personality,
                    &pending.tool,
                    &applied,
                    &mut turn,
                )
                .await?;
                return Ok(Some(content));
            }
            if is_soft_constraint_phrase(text) {
                let merged = merge_free_text_into_tool_input(&pending.tool_input, text);
                let merged = set_apply_flag(&merged, false);
                let content = dispatch_tool(
                    app,
                    state,
                    ctx,
                    conversation_id,
                    &goal,
                    personality,
                    &pending.tool,
                    &merged,
                    &mut turn,
                )
                .await?;
                return Ok(Some(content));
            }
            // Unrelated message — leave job open and fall through to classify.
            Ok(None)
        }
        JobPhase::Running => Ok(None),
    }
}

pub(crate) enum ToolStepOutcome {
    NeedsUser(String),
    Done { output: String, content: String },
    Failed { output: String, content: String },
}

#[allow(clippy::too_many_arguments)]
async fn try_canonical_route(
    app: &AppHandle,
    state: &AppState,
    ctx: &MemoryContext,
    conversation_id: &str,
    text: &str,
    personality: &PersonalityProfile,
    trace: &mut TurnTrace,
) -> Result<Option<String>, String> {
    let Route::Tools(jobs) = state.plugins.route(text) else {
        return Ok(None);
    };
    if jobs.is_empty() {
        return Ok(None);
    }
    emit_trace(app, "running", "Canonical tool");
    state.memory.clear_agent_turn(conversation_id);
    let mut turn = AgentTurn {
        goal: text.to_string(),
        scratchpad: Vec::new(),
    };
    let mut replies: Vec<String> = Vec::new();
    let mut used_tools: Vec<String> = Vec::new();
    let mut last_doc: Option<String> = None;
    for job in jobs {
        match execute_tool_step(
            app,
            state,
            ctx,
            conversation_id,
            text,
            personality,
            &job.tool,
            &job.input,
            &turn,
        )
        .await?
        {
            ToolStepOutcome::NeedsUser(content) => {
                trace.clarification_count += 1;
                return Ok(Some(content));
            }
            ToolStepOutcome::Done { output, content }
            | ToolStepOutcome::Failed { output, content } => {
                turn.scratchpad.push(ScratchStep {
                    tool: job.tool.clone(),
                    summary: compact_tool_summary(&job.tool, &output),
                });
                used_tools.push(job.tool.clone());
                trace.tool_steps += 1;
                if job.tool.starts_with("docs.") {
                    last_doc = Some(output);
                }
                replies.push(content);
            }
        }
    }
    native_loop::emit_workspace_focus(app, &used_tools, last_doc.as_deref());
    if !replies.is_empty() {
        let _ = app.emit("chat-chunk", &replies.join("\n"));
    }
    Ok(Some(replies.join("\n")))
}

/// Validate → Core → phrase. Used by canonical route, clarification, and the native loop.
pub(crate) async fn execute_tool_step(
    app: &AppHandle,
    state: &AppState,
    ctx: &MemoryContext,
    conversation_id: &str,
    text: &str,
    personality: &PersonalityProfile,
    tool_name: &str,
    raw_input: &str,
    turn: &AgentTurn,
) -> Result<ToolStepOutcome, String> {
    let session = SessionContext {
        conversation_id: conversation_id.to_string(),
        workspace_path: Some(state.project_root.display().to_string()),
        user_message: Some(text.to_string()),
    };
    let mut tool_input = merge_session_into_input(raw_input, &session);
    if let Some(pending) = state.memory.get_pending_clarification(conversation_id) {
        if pending.tool == tool_name {
            tool_input = merge_tool_input(&pending.tool_input, &tool_input);
        }
    }

    let prefs = DbPreferenceLookup { db: &state.db };
    let clarify_cfg = load_clarification_config(state);
    let schema = state.tool_schema(tool_name);

    match clarify(tool_name, &tool_input, schema, &prefs, &clarify_cfg) {
        ClarifyResult::NeedsInput {
            tool_input: partial,
            missing,
            context_hint,
        } => {
            emit_trace(app, "clarifying", "Need more information");
            let mut pending = PendingClarification::from_needs(
                tool_name,
                partial,
                missing.clone(),
                conversation_id,
            );
            pending.agent_goal = Some(turn.goal.clone());
            pending.agent_scratchpad = serde_json::to_string(&turn.scratchpad).ok();
            state
                .memory
                .set_pending_clarification(conversation_id, pending);
            let labels: Vec<String> = missing.iter().map(|m| m.label.clone()).collect();
            let question = phrase_clarification(
                personality,
                &ClarificationAsk {
                    field_labels: labels,
                    context_hint,
                },
            );
            let content = style_response(personality, &question);
            emit_ask(app, tool_name, &content, missing.first());
            let _ = app.emit("chat-chunk", &content);
            Ok(ToolStepOutcome::NeedsUser(content))
        }
        ClarifyResult::Ready {
            tool_input: ready_input,
        } => {
            state.memory.clear_pending_clarification(conversation_id);
            emit_trace(app, "running", &format!("Running {tool_name}"));
            let start = Instant::now();
            match run_tool_with_tracking(state, app, ctx, tool_name, &ready_input, start) {
                Ok(output) => {
                    info!(tool = %tool_name, "tool executed");
                    if is_calendar_write_conflict(tool_name, &output) {
                        state.memory.set_pending_clarification(
                            conversation_id,
                            PendingClarification {
                                tool: tool_name.to_string(),
                                tool_input: ready_input.clone(),
                                missing: vec![MissingField {
                                    name: "force".into(),
                                    label: "force".into(),
                                    ask_kind: AskKind::Text,
                                    choices: vec![],
                                }],
                                missing_labels: vec!["force".into()],
                                conversation_id: conversation_id.to_string(),
                                follow_up: false,
                                agent_scratchpad: serde_json::to_string(&turn.scratchpad).ok(),
                                agent_goal: Some(turn.goal.clone()),
                                phase: JobPhase::NeedsInput,
                                last_proposal: None,
                            },
                        );
                        let content =
                            style_response(personality, &phrase_tool_result(tool_name, &output));
                        let _ = app.emit("chat-chunk", &content);
                        return Ok(ToolStepOutcome::NeedsUser(content));
                    }

                    if let Some((content, _)) = maybe_emit_tool_ask(
                        app,
                        state,
                        personality,
                        conversation_id,
                        tool_name,
                        &output,
                        turn,
                    ) {
                        return Ok(ToolStepOutcome::NeedsUser(content));
                    }

                    if looks_like_proposal(tool_name, &ready_input, &output) {
                        if let Some(proposal) = crate::calendar_proposal::proposal_from_organize(
                            conversation_id,
                            &ready_input,
                            &output,
                        ) {
                            crate::calendar_proposal::save_proposal(state, &proposal);
                            crate::calendar_proposal::emit_proposal(app, &proposal);
                        }
                        state.memory.set_pending_clarification(
                            conversation_id,
                            PendingClarification {
                                tool: tool_name.to_string(),
                                tool_input: set_apply_flag(&ready_input, false),
                                missing: vec![],
                                missing_labels: vec![],
                                conversation_id: conversation_id.to_string(),
                                follow_up: false,
                                agent_scratchpad: serde_json::to_string(&turn.scratchpad).ok(),
                                agent_goal: Some(turn.goal.clone()),
                                phase: JobPhase::AwaitingConfirm,
                                last_proposal: Some(output.clone()),
                            },
                        );
                        let content =
                            style_response(personality, &phrase_tool_result(tool_name, &output));
                        let _ = app.emit("chat-chunk", &content);
                        return Ok(ToolStepOutcome::NeedsUser(content));
                    }

                    let content =
                        style_response(personality, &phrase_tool_result(tool_name, &output));
                    Ok(ToolStepOutcome::Done { output, content })
                }
                Err(err_msg) => {
                    let output = serde_json::json!({ "error": err_msg }).to_string();
                    let content =
                        style_response(personality, &format!("Tool execution failed: {err_msg}"));
                    Ok(ToolStepOutcome::Failed { output, content })
                }
            }
        }
    }
}

async fn dispatch_tool(
    app: &AppHandle,
    state: &AppState,
    ctx: &MemoryContext,
    conversation_id: &str,
    text: &str,
    personality: &PersonalityProfile,
    tool_name: &str,
    raw_input: &str,
    turn: &mut AgentTurn,
) -> Result<String, String> {
    match execute_tool_step(
        app,
        state,
        ctx,
        conversation_id,
        text,
        personality,
        tool_name,
        raw_input,
        turn,
    )
    .await?
    {
        ToolStepOutcome::NeedsUser(content) => {
            save_agent_turn(state, conversation_id, turn);
            Ok(content)
        }
        ToolStepOutcome::Done { output, content }
        | ToolStepOutcome::Failed { output, content } => {
            turn.scratchpad.push(ScratchStep {
                tool: tool_name.to_string(),
                summary: compact_tool_summary(tool_name, &output),
            });
            save_agent_turn(state, conversation_id, turn);
            if calendar_write_applied(tool_name, &output)
                || tool_name == "calendar.look"
                || tool_name == "calendar.find_free_time"
            {
                if state
                    .memory
                    .get_pending_clarification(conversation_id)
                    .is_none()
                {
                    state.memory.clear_agent_turn(conversation_id);
                }
            }
            let _ = app.emit("chat-chunk", &content);
            Ok(content)
        }
    }
}

/// Resolve a structured clarification answer and continue the agent loop.
#[instrument(skip(state, app), fields(conversation_id = %conversation_id))]
pub async fn resolve_clarification(
    app: AppHandle,
    state: &AppState,
    conversation_id: String,
    field: String,
    value: String,
) -> Result<(), String> {
    let Some(pending) = state.memory.get_pending_clarification(&conversation_id) else {
        return Err("No pending clarification to resolve.".into());
    };

    let started = Instant::now();
    let mut trace = TurnTrace::new(&conversation_id);
    trace.clarification_count = 1;

    let _scope = RunScope::start(&state.runs, &conversation_id);

    emit_trace(&app, "clarifying", &format!("Resolved {field}"));

    let mut turn = AgentTurn {
        goal: pending
            .agent_goal
            .clone()
            .unwrap_or_else(|| load_agent_turn(state, &conversation_id, "").goal),
        scratchpad: pending
            .agent_scratchpad
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default(),
    };
    if turn.goal.is_empty() {
        turn = load_agent_turn(state, &conversation_id, "");
    }

    // Follow-up asks (tool `_ask`): merge selection into context, continue loop.
    if pending.follow_up {
        // Deterministic free-slot → create_event (user already chose the time).
        if (pending.tool == "calendar.look" || pending.tool == "calendar.find_free_time")
            && (field == "selected_slot"
                || pending.missing.iter().any(|m| m.name == "selected_slot"))
        {
            if let Some(create_input) =
                pin_input_from_selected_slot(&turn.goal, &value)
            {
                turn.scratchpad.push(ScratchStep {
                    tool: "calendar.look._ask".into(),
                    summary: "User selected slot → pin".into(),
                });
                state
                    .memory
                    .clear_pending_clarification(&conversation_id);
                save_agent_turn(state, &conversation_id, &turn);

                let ctx = state.memory.ctx(&conversation_id);
                let personality = load_personality(state);
                let goal = turn.goal.clone();

                let content = dispatch_tool(
                    &app,
                    state,
                    &ctx,
                    &conversation_id,
                    &goal,
                    &personality,
                    "calendar.pin",
                    &create_input,
                    &mut turn,
                )
                .await?;

                state
                    .db
                    .add_message_with_metadata(
                        &conversation_id,
                        "assistant",
                        &content,
                        Some(
                            &json!({
                                "intent": "tool_use",
                                "tool": "calendar.pin",
                                "resolved_field": field,
                                "follow_up": true,
                                "from_free_slot": true,
                            })
                            .to_string(),
                        ),
                    )
                    .map_err(|e| e.to_string())?;
                if state
                    .memory
                    .get_pending_clarification(&conversation_id)
                    .is_none()
                {
                    state.memory.clear_agent_turn(&conversation_id);
                }
                trace.tool_steps += 1;
                seal_trace(&app, state, &mut trace, TurnPath::OpenJob, started);
                let _ = app.emit("chat-done", ());
                return Ok(());
            }
        }

        let label = pending
            .missing
            .iter()
            .flat_map(|m| m.choices.iter())
            .find(|c| c.value == value || c.id == value)
            .map(|c| c.label.clone())
            .unwrap_or_else(|| value.clone());
        turn.scratchpad.push(ScratchStep {
            tool: format!("{}._ask", pending.tool),
            summary: format!("User chose: {label} ({value})"),
        });
        save_agent_turn(state, &conversation_id, &turn);
    }

    let merged = merge_field_value(&pending.tool_input, &field, &value);
    let tool_name = pending.tool.clone();
    state
        .memory
        .clear_pending_clarification(&conversation_id);

    state.memory.set_pending_clarification(
        &conversation_id,
        PendingClarification {
            tool: tool_name.clone(),
            tool_input: merged.clone(),
            missing: vec![],
            missing_labels: vec![],
            conversation_id: conversation_id.clone(),
            follow_up: false,
            agent_scratchpad: serde_json::to_string(&turn.scratchpad).ok(),
            agent_goal: Some(turn.goal.clone()),
            phase: JobPhase::NeedsInput,
            last_proposal: pending.last_proposal.clone(),
        },
    );

    let ctx = state.memory.ctx(&conversation_id);
    let personality = load_personality(state);
    let goal = turn.goal.clone();

    let content = dispatch_tool(
        &app,
        state,
        &ctx,
        &conversation_id,
        &goal,
        &personality,
        &tool_name,
        &merged,
        &mut turn,
    )
    .await?;

    state
        .db
        .add_message_with_metadata(
            &conversation_id,
            "assistant",
            &content,
            Some(
                &json!({
                    "intent": "tool_use",
                    "tool": tool_name,
                    "resolved_field": field,
                })
                .to_string(),
            ),
        )
        .map_err(|e| e.to_string())?;
    if state
        .memory
        .get_pending_clarification(&conversation_id)
        .is_none()
    {
        state.memory.clear_agent_turn(&conversation_id);
    }
    trace.tool_steps += 1;
    seal_trace(&app, state, &mut trace, TurnPath::OpenJob, started);
    let _ = app.emit("chat-done", ());
    Ok(())
}

async fn stream_talk_reply(
    app: &AppHandle,
    state: &AppState,
    client: &reqwest::Client,
    text: &str,
    history: &[HistoryMessage],
    memory: &BrainMemoryContext,
    personality: &PersonalityProfile,
    run: &RunGuard,
    trace: &mut TurnTrace,
) -> Result<String, String> {
    if let Some(pm) = app.try_state::<Arc<ProcessManager>>() {
        emit_trace(app, "planning", "Checking Brain");
        if let Err(e) = pm.ensure_brain(state).await {
            warn!(error = %e, "talk ensure_brain failed");
        }
    }

    wake_chat_mlx(app, state).await;

    let mut recycled = false;
    loop {
        emit_trace(app, "responding", "Llama chat");
        match try_stream_talk(app, state, client, text, history, personality, run).await {
            Ok(content) => {
                trace.set_path(TurnPath::TalkLlama);
                trace.model = Some("llama".into());
                trace.model_call_count += 1;
                return Ok(content);
            }
            Err(err) => {
                warn!(error = %err, recycled, "talk stream failed");
                match talk_recovery(&err, recycled) {
                    TalkRecovery::RecycleBrain => {
                        emit_trace(app, "planning", "Restarting Brain");
                        if let Some(pm) = app.try_state::<Arc<ProcessManager>>() {
                            if let Err(e) = pm.recycle_brain(state).await {
                                warn!(error = %e, "brain recycle failed");
                            }
                        }
                        recycled = true;
                    }
                    TalkRecovery::FallbackRespond => {
                        emit_trace(app, "responding", "Talk fallback");
                        trace.set_path(TurnPath::TalkRespond);
                        trace.model = Some("qwen".into());
                        trace.model_call_count += 1;
                        return stream_chat_reply(
                            app,
                            state,
                            client,
                            text,
                            history,
                            memory,
                            personality,
                            run,
                            false,
                        )
                        .await;
                    }
                    TalkRecovery::GiveUp => return Err(err),
                }
            }
        }
    }
}

async fn try_stream_talk(
    app: &AppHandle,
    state: &AppState,
    client: &reqwest::Client,
    text: &str,
    history: &[HistoryMessage],
    personality: &PersonalityProfile,
    run: &RunGuard,
) -> Result<String, String> {
    let recent: Vec<HistoryMessage> = history
        .iter()
        .rev()
        .filter(|m| (m.role == "user" || m.role == "assistant") && !m.content.trim().is_empty())
        .take(8)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let send = client.post(format!("{}/chat/talk", state.brain_url())).json(
        &json!({
            "message": text,
            "history": recent,
            "model": ProcessManager::chat_model(state),
        }),
    );
    let resp = tokio::select! {
        _ = run.cancelled() => {
            let content = style_response(personality, "Stopped.");
            let _ = app.emit("chat-chunk", &content);
            return Ok(content);
        }
        resp = send.send() => {
            resp.map_err(|e| format!("brain talk request failed: {e}"))?
        }
    };
    if !resp.status().is_success() {
        return Err(format!("brain talk HTTP {}", resp.status()));
    }
    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut assistant_content = String::new();
    let mut got_token = false;
    let first_token = tokio::time::sleep(Duration::from_secs(90));
    tokio::pin!(first_token);
    loop {
        tokio::select! {
            _ = run.cancelled() => break,
            _ = &mut first_token, if !got_token => {
                warn!("talk first token timeout");
                break;
            }
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(bytes)) => {
                        let piece = String::from_utf8_lossy(&bytes);
                        if !piece.is_empty() {
                            got_token = true;
                            assistant_content.push_str(&piece);
                            let _ = app.emit("chat-chunk", piece.as_ref());
                        }
                    }
                    Some(Err(e)) => {
                        error!(error = %e, "talk stream error");
                        break;
                    }
                    None => break,
                }
            }
        }
    }
    if assistant_content.trim().is_empty() {
        let content = if run.is_cancelled() {
            style_response(personality, "Stopped.")
        } else if !got_token {
            style_response(
                personality,
                "Llama chat stalled. Try again — first load can take a moment.",
            )
        } else {
            style_response(personality, "I'm here — try asking again.")
        };
        let _ = app.emit("chat-chunk", &content);
        return Ok(content);
    }
    Ok(assistant_content)
}

async fn stream_chat_reply(
    app: &AppHandle,
    state: &AppState,
    client: &reqwest::Client,
    text: &str,
    history: &[HistoryMessage],
    memory: &BrainMemoryContext,
    personality: &PersonalityProfile,
    run: &RunGuard,
    wake: bool,
) -> Result<String, String> {
    if wake {
        wake_mlx(app, state).await;
    }
    emit_trace(
        app,
        "responding",
        if wake {
            "Writing reply"
        } else {
            "Talk fallback"
        },
    );
    let send = client
        .post(format!("{}/chat/respond", state.brain_url()))
        .json(&RespondRequest {
            message: text.to_string(),
            history: history.to_vec(),
            memory: memory.clone(),
            tool_name: None,
            tool_result: None,
        });
    let resp = tokio::select! {
        _ = run.cancelled() => {
            let content = style_response(personality, "Stopped.");
            let _ = app.emit("chat-chunk", &content);
            return Ok(content);
        }
        resp = send.send() => {
            resp.map_err(|e| format!("brain respond request failed: {e}"))?
        }
    };
    if !resp.status().is_success() {
        return Err(format!("brain respond HTTP {}", resp.status()));
    }
    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut assistant_content = String::new();
    let mut got_token = false;
    let first_token = tokio::time::sleep(Duration::from_secs(90));
    tokio::pin!(first_token);
    loop {
        tokio::select! {
            _ = run.cancelled() => break,
            _ = &mut first_token, if !got_token => {
                warn!("chat first token timeout");
                break;
            }
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(bytes)) => {
                        let piece = String::from_utf8_lossy(&bytes);
                        if !piece.is_empty() {
                            got_token = true;
                            assistant_content.push_str(&piece);
                            let _ = app.emit("chat-chunk", piece.as_ref());
                        }
                    }
                    Some(Err(e)) => {
                        error!(error = %e, "stream error");
                        break;
                    }
                    None => break,
                }
            }
        }
    }
    if assistant_content.trim().is_empty() {
        let content = if run.is_cancelled() {
            style_response(personality, "Stopped.")
        } else if !got_token {
            style_response(
                personality,
                "The local model stalled before answering. Try again.",
            )
        } else {
            style_response(personality, "I'm here — try asking again.")
        };
        let _ = app.emit("chat-chunk", &content);
        return Ok(content);
    }
    Ok(assistant_content)
}

fn persist_assistant_turn(
    app: &AppHandle,
    state: &AppState,
    ctx: &MemoryContext,
    conversation_id: &str,
    content: &str,
    metadata: &str,
) -> Result<(), String> {
    state
        .db
        .add_message_with_metadata(conversation_id, "assistant", content, Some(metadata))
        .map_err(|e| e.to_string())?;
    let _ = state.memory.store_event(
        ctx,
        MemoryEvent::MessageAdded {
            role: "assistant".into(),
            content: content.to_string(),
        },
    );
    let _ = app.emit("chat-done", ());
    Ok(())
}

fn persist_stopped(
    app: &AppHandle,
    state: &AppState,
    ctx: &MemoryContext,
    conversation_id: &str,
    personality: &PersonalityProfile,
) -> Result<(), String> {
    state.memory.clear_agent_turn(conversation_id);
    let content = style_response(personality, "Stopped.");
    let _ = app.emit("chat-chunk", &content);
    persist_assistant_turn(
        app,
        state,
        ctx,
        conversation_id,
        &content,
        &json!({"intent":"stopped"}).to_string(),
    )
}

async fn cancellable_json<T: serde::de::DeserializeOwned>(
    run: &RunGuard,
    request: reqwest::RequestBuilder,
) -> Result<T, String> {
    let http = async {
        let resp = request
            .send()
            .await
            .map_err(|e| format!("brain request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("brain HTTP {}", resp.status()));
        }
        resp.json::<T>()
            .await
            .map_err(|e| format!("brain parse failed: {e}"))
    };
    tokio::select! {
        _ = run.cancelled() => Err("stopped".into()),
        res = http => res,
    }
}

fn seal_trace(
    app: &AppHandle,
    state: &AppState,
    trace: &mut TurnTrace,
    path: TurnPath,
    started: Instant,
) {
    trace.set_path(path);
    turn_trace::finish_turn_trace(app, state, trace, started);
}

pub(crate) fn emit_trace(app: &AppHandle, step: &str, detail: &str) {
    let _ = app.emit(
        "chat-trace",
        json!({
            "step": step,
            "detail": detail,
        }),
    );
}

pub(crate) fn emit_ask(app: &AppHandle, tool: &str, question: &str, primary: Option<&MissingField>) {
    let (field, ask_kind, options) = match primary {
        Some(m) => (
            m.name.clone(),
            match m.ask_kind {
                AskKind::Choice => "choice",
                AskKind::Text => "text",
            }
            .to_string(),
            m.choices.clone(),
        ),
        None => (String::new(), "text".into(), vec![]),
    };
    let _ = app.emit(
        "chat-ask",
        json!({
            "tool": tool,
            "question": question,
            "field": field,
            "ask_kind": ask_kind,
            "options": options,
        }),
    );
}

/// If tool output contains `_ask`, park a follow-up choice and emit chat-ask.
pub(crate) fn maybe_emit_tool_ask(
    app: &AppHandle,
    state: &AppState,
    personality: &PersonalityProfile,
    conversation_id: &str,
    tool_name: &str,
    output: &str,
    turn: &AgentTurn,
) -> Option<(String, ())> {
    let value: serde_json::Value = serde_json::from_str(output).ok()?;
    let ask = value.get("_ask")?;
    let prompt = ask.get("prompt")?.as_str()?.to_string();
    let field = ask.get("field")?.as_str()?.to_string();
    let options_raw = ask.get("options")?.as_array()?;
    let choices: Vec<AskChoice> = options_raw
        .iter()
        .filter_map(|o| {
            Some(AskChoice {
                id: o.get("id")?.as_str()?.to_string(),
                label: o.get("label")?.as_str()?.to_string(),
                value: match o.get("value") {
                    Some(serde_json::Value::String(s)) => s.clone(),
                    Some(other) => other.to_string(),
                    None => return None,
                },
            })
        })
        .collect();
    if choices.is_empty() {
        return None;
    }

    let missing = vec![MissingField::from_dynamic_ask(
        field.clone(),
        prompt.clone(),
        choices.clone(),
    )];
    let mut partial = value.clone();
    if let Some(obj) = partial.as_object_mut() {
        obj.remove("_ask");
    }
    state.memory.set_pending_clarification(
        conversation_id,
        PendingClarification {
            tool: tool_name.to_string(),
            tool_input: partial.to_string(),
            missing_labels: vec![prompt.clone()],
            missing: missing.clone(),
            conversation_id: conversation_id.to_string(),
            follow_up: true,
            agent_scratchpad: serde_json::to_string(&turn.scratchpad).ok(),
            agent_goal: Some(turn.goal.clone()),
            phase: JobPhase::NeedsInput,
            last_proposal: None,
        },
    );

    let phrased = phrase_tool_result(tool_name, output);
    let content = style_response(personality, &format!("{phrased}\n\n{prompt}"));
    emit_ask(app, tool_name, &content, missing.first());
    let _ = app.emit("chat-chunk", &content);
    Some((content, ()))
}

#[instrument(skip(state), fields(conversation_id = %conversation_id))]
pub async fn delete_conversation(state: &AppState, conversation_id: &str) -> Result<(), String> {
    let conversation = match state.db.get_conversation(conversation_id) {
        Ok(conv) => conv,
        Err(DbError::NotFound(_)) => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };

    let messages = state
        .db
        .get_messages(conversation_id)
        .map_err(|e| e.to_string())?;
    let history: Vec<HistoryMessage> = messages
        .iter()
        .map(|m| HistoryMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    memory_api::archive_conversation(state, conversation_id, &conversation.title, &history).await;
    state.memory.clear_pending_clarification(conversation_id);
    state.memory.clear_agent_turn(conversation_id);
    state.memory.clear_last_look(conversation_id);
    state.memory.clear_last_life_look(conversation_id);

    match state.db.delete_conversation(conversation_id) {
        Ok(()) => {}
        Err(DbError::NotFound(_)) => return Ok(()),
        Err(e) => return Err(e.to_string()),
    }
    info!(conversation_id = %conversation_id, "conversation deleted");
    Ok(())
}

/// Spark delete from UI — Core tool path archives; this is a direct DB delete
/// with Memory archive for Settings UI deletes.
#[instrument(skip(state, app), fields(spark_id = %spark_id))]
pub async fn delete_spark_with_archive(
    state: &AppState,
    app: &AppHandle,
    spark_id: &str,
) -> Result<(), String> {
    let input = serde_json::json!({"id": spark_id, "action": "delete"}).to_string();
    let start = Instant::now();
    let ctx = state.memory.ctx("spark-ui");
    run_tool_with_tracking(state, app, &ctx, "update_spark", &input, start)?;
    Ok(())
}

pub(crate) fn run_tool_with_tracking(
    state: &AppState,
    app: &AppHandle,
    ctx: &MemoryContext,
    tool_name: &str,
    tool_input: &str,
    start: Instant,
) -> Result<String, String> {
    match state.task_runner.run(tool_name, tool_input) {
        Ok(run_result) => {
            let output = run_result.output.clone();
            let duration_ms = start.elapsed().as_millis() as u64;
            if let Ok(r) = state.memory.store_event(
                ctx,
                MemoryEvent::ToolExecuted {
                    tool: tool_name.to_string(),
                    params: tool_input.to_string(),
                    result: output.clone(),
                    duration_ms,
                    success: true,
                },
            ) {
                state.memory.spawn_index_saved(ctx, &r.saved);
            }
            match buddy_plugins::after_execute_hint(tool_name) {
                buddy_core::AfterExecute::EmitSparksUpdated => emit_spark_updates(app, state),
                buddy_core::AfterExecute::EmitCalendarUpdated => {
                    let is_conflict = serde_json::from_str::<serde_json::Value>(&output)
                        .ok()
                        .and_then(|v| v.get("status")?.as_str().map(|s| s == "conflict"))
                        .unwrap_or(false);
                    if !is_conflict {
                        let _ = app.emit("calendar-updated", ());
                    }
                }
                buddy_core::AfterExecute::EmitTodosUpdated => {
                    let _ = app.emit("todos-updated", ());
                }
                buddy_core::AfterExecute::EmitDocsUpdated => {
                    let _ = app.emit("docs-updated", ());
                }
                buddy_core::AfterExecute::EmitResearchUpdated => {
                    let _ = app.emit("research-updated", ());
                }
                buddy_core::AfterExecute::EmitStudyUpdated => {
                    let _ = app.emit("study-updated", ());
                }
                buddy_core::AfterExecute::EmitSocialsUpdated => {
                    let _ = app.emit("socials-updated", ());
                    if tool_name == "socials.commit_approved" {
                        let _ = app.emit("calendar-updated", ());
                    }
                }
                buddy_core::AfterExecute::EmitFitnessUpdated => {
                    let _ = app.emit("fitness-updated", ());
                }
                buddy_core::AfterExecute::EmitMoneyUpdated => {
                    let _ = app.emit("money-updated", ());
                }
                buddy_core::AfterExecute::None => {}
            }
            Ok(output)
        }
        Err(e) => {
            let err_msg = e.to_string();
            let duration_ms = start.elapsed().as_millis() as u64;
            let _ = state.memory.store_event(
                ctx,
                MemoryEvent::ToolExecuted {
                    tool: tool_name.to_string(),
                    params: tool_input.to_string(),
                    result: err_msg.clone(),
                    duration_ms,
                    success: false,
                },
            );
            let _ = state.memory.store_event(
                ctx,
                MemoryEvent::ToolFailed {
                    error: err_msg.clone(),
                    cause: err_msg.clone(),
                    resolution: None,
                },
            );
            Err(err_msg)
        }
    }
}

fn emit_spark_updates(app: &AppHandle, state: &AppState) {
    let count = state
        .db
        .count_stale_sparks(SPARK_STALE_AGE_MS, SPARK_NUDGE_COOLDOWN_MS)
        .unwrap_or(0);
    let _ = app.emit("sparks-stale", count);
    let _ = app.emit("sparks-updated", ());
}

fn load_personality(state: &AppState) -> PersonalityProfile {
    let raw = state
        .db
        .get_setting("personality_profile_json")
        .ok()
        .flatten();
    PersonalityProfile::from_settings_json(raw.as_deref())
}

fn load_clarification_config(state: &AppState) -> ClarificationConfig {
    let threshold = state
        .db
        .get_setting("clarification_confidence_threshold")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.75);
    ClarificationConfig {
        confidence_threshold: threshold.clamp(0.0, 1.0),
    }
}

fn merge_tool_input(previous: &str, next: &str) -> String {
    let prev: serde_json::Value =
        serde_json::from_str(previous).unwrap_or_else(|_| serde_json::json!({}));
    let newv: serde_json::Value =
        serde_json::from_str(next).unwrap_or_else(|_| serde_json::json!({}));
    match (prev.as_object(), newv.as_object()) {
        (Some(p), Some(n)) => {
            let mut merged = p.clone();
            for (k, v) in n {
                let empty = match v {
                    serde_json::Value::Null => true,
                    serde_json::Value::String(s) => s.trim().is_empty(),
                    _ => false,
                };
                if !empty {
                    merged.insert(k.clone(), v.clone());
                }
            }
            serde_json::Value::Object(merged).to_string()
        }
        _ => next.to_string(),
    }
}

fn is_force_confirm(text: &str) -> bool {
    matches!(
        text.trim().to_ascii_lowercase().as_str(),
        "allow"
            | "force"
            | "force it"
            | "yes"
            | "y"
            | "ok"
            | "okay"
            | "go ahead"
            | "override"
            | "do it"
            | "book it"
            | "book it anyway"
            | "schedule it anyway"
    )
}

fn is_force_cancel(text: &str) -> bool {
    matches!(
        text.trim().to_ascii_lowercase().as_str(),
        "no" | "cancel" | "never mind" | "nevermind" | "don't" | "do not" | "stop"
    )
}

fn calendar_write_applied(tool: &str, output: &str) -> bool {
    if !tool.starts_with("calendar.") {
        return false;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(output) else {
        return false;
    };
    // schedule_task / plan_day / block_time stamp apply on the result when writing.
    if v.get("apply").and_then(|a| a.as_bool()) == Some(true) {
        return true;
    }
    false
}

/// Build `calendar.pin` input from a free-slot choice value and the turn goal.
fn pin_input_from_selected_slot(goal: &str, slot_value: &str) -> Option<String> {
    let slot: serde_json::Value = serde_json::from_str(slot_value.trim()).ok()?;
    let start = slot
        .get("start")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))?;
    let end = slot
        .get("end")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))?;
    if end <= start {
        return None;
    }
    let title = title_from_free_time_goal(goal);
    Some(
        json!({
            "action": "create",
            "title": title,
            "start": start,
            "end": end,
        })
        .to_string(),
    )
}

fn title_from_free_time_goal(goal: &str) -> String {
    let lower = goal.trim().to_ascii_lowercase();

    // Prefer explicit "for <activity>" when present (skip pure duration fragments).
    if let Some(idx) = lower.rfind(" for ") {
        let after = goal.trim()[idx + 5..].trim();
        let cleaned = strip_trailing_time_noise(after);
        let cl = cleaned.to_ascii_lowercase();
        let looks_like_duration = cl.contains("hour")
            || cl.contains("hr")
            || cl.contains("min")
            || cleaned.chars().all(|c| c.is_ascii_digit() || c.is_whitespace());
        if cleaned.len() >= 2 && !looks_like_duration {
            return title_case_first(&cleaned);
        }
    }

    let mut t = goal.trim().to_string();
    // Longer phrases first so "when am i free for" wins over "when am i free".
    for pat in [
        "when am i free for",
        "when am i free",
        "find free time for",
        "find free time",
        "got any free time for",
        "got any free time",
        "any free time for",
        "any free time",
        "any open slots for",
        "any open slots",
        "am i free for",
        "am i free",
        "free for",
    ] {
        let lower = t.to_ascii_lowercase();
        if let Some(idx) = lower.find(pat) {
            t = t[idx + pat.len()..].trim().to_string();
            break;
        }
    }

    let cleaned = strip_trailing_time_noise(&t);
    if cleaned.len() < 2 {
        "Focus time".into()
    } else {
        title_case_first(&cleaned)
    }
}

fn strip_trailing_time_noise(text: &str) -> String {
    let mut t = text.trim().to_string();
    let lower = t.to_ascii_lowercase();
    for stop in [
        " tomorrow",
        " today",
        " this week",
        " next week",
        " for ",
    ] {
        if let Some(idx) = lower.find(stop) {
            t = t[..idx].trim().to_string();
            break;
        }
    }
    // Bare day tokens left after stripping the free-time phrase.
    let bare = t.trim().trim_matches(|c: char| {
        c == '?' || c == '.' || c == ',' || c == ':' || c.is_whitespace()
    });
    let bare_l = bare.to_ascii_lowercase();
    if matches!(
        bare_l.as_str(),
        "today" | "tomorrow" | "tonight" | "this week" | "next week" | ""
    ) {
        return String::new();
    }
    // Drop leading day words: "tomorrow climbing" → "climbing"
    let mut out = bare.to_string();
    for prefix in ["tomorrow ", "today ", "tonight "] {
        let ol = out.to_ascii_lowercase();
        if ol.starts_with(prefix) {
            out = out[prefix.len()..].trim().to_string();
            break;
        }
    }
    out.trim()
        .trim_matches(|c: char| c == '?' || c == '.' || c == ',' || c == ':' || c.is_whitespace())
        .to_string()
}

fn title_case_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
        None => "Focus time".into(),
    }
}

fn pending_is_force(pending: &PendingClarification) -> bool {
    pending.labels().iter().any(|l| {
        let t = l.to_ascii_lowercase();
        t == "force" || t.contains("override") || t.contains("conflict")
    }) && (pending.tool == "calendar.create_event"
        || pending.tool == "calendar.update_event"
        || pending.tool == "calendar.pin")
}

pub(crate) fn is_calendar_write_conflict(tool_name: &str, output: &str) -> bool {
    if tool_name != "calendar.create_event"
        && tool_name != "calendar.update_event"
        && tool_name != "calendar.pin"
    {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(output)
        .ok()
        .and_then(|v| v.get("status")?.as_str().map(|s| s == "conflict"))
        .unwrap_or(false)
}

#[allow(clippy::too_many_arguments)]
async fn try_force_confirm_pending(
    app: &AppHandle,
    state: &AppState,
    _client: &reqwest::Client,
    ctx: &MemoryContext,
    conversation_id: &str,
    text: &str,
    _history: &[HistoryMessage],
    _memory: &BrainMemoryContext,
    personality: &PersonalityProfile,
) -> Result<Option<(String, String)>, String> {
    let Some(pending) = state.memory.get_pending_clarification(conversation_id) else {
        return Ok(None);
    };
    if !pending_is_force(&pending) {
        return Ok(None);
    }

    if is_force_cancel(text) {
        state.memory.clear_pending_clarification(conversation_id);
        let content = style_response(personality, "Okay — I won’t override that conflict.");
        let _ = app.emit("chat-chunk", &content);
        return Ok(Some((content, pending.tool)));
    }

    if !is_force_confirm(text) {
        return Ok(None);
    }

    let forced_input = merge_tool_input(&pending.tool_input, r#"{"force":true}"#);
    let tool_name = pending.tool.clone();
    // Clear before dispatch so Ready path doesn't reuse stale pending.
    state.memory.clear_pending_clarification(conversation_id);
    let mut turn = load_agent_turn(state, conversation_id, text);
    let content = dispatch_tool(
        app,
        state,
        ctx,
        conversation_id,
        text,
        personality,
        &tool_name,
        &forced_input,
        &mut turn,
    )
    .await?;
    Ok(Some((content, tool_name)))
}

struct DbPreferenceLookup<'a> {
    db: &'a buddy_database::Database,
}

impl PreferenceLookup for DbPreferenceLookup<'_> {
    fn get(&self, key: &str) -> Option<(String, f64)> {
        self.db
            .get_setting(key)
            .ok()
            .flatten()
            .filter(|v| !v.trim().is_empty())
            .map(|v| (v, 0.9))
    }
}

#[cfg(test)]
mod free_slot_book_tests {
    use super::*;

    #[test]
    fn selected_slot_builds_create_event() {
        let input = pin_input_from_selected_slot(
            "When am I free tomorrow for climbing?",
            r#"{"start":1000,"end":2000}"#,
        )
        .expect("slot");
        let v: serde_json::Value = serde_json::from_str(&input).unwrap();
        assert_eq!(v["start"], 1000);
        assert_eq!(v["end"], 2000);
        assert!(v["title"].as_str().unwrap().to_ascii_lowercase().contains("climb"));
    }

    #[test]
    fn rejects_inverted_slot() {
        assert!(pin_input_from_selected_slot(
            "free time",
            r#"{"start":2000,"end":1000}"#,
        )
        .is_none());
    }

    #[test]
    fn default_title_when_goal_is_generic() {
        assert_eq!(title_from_free_time_goal("Any open slots today?"), "Focus time");
    }
}
