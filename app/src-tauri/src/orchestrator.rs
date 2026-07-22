//! Buddy turn router: Memory → Brain → Clarification → Core → Memory → UI.
//!
//! Routing only. No planning, no tool-name branches, no temporary state ownership.

use std::time::Instant;

use buddy_clarification::{
    clarify, ClarifyResult, ClarificationConfig, PendingClarification, PreferenceLookup,
};
use buddy_core::{merge_session_into_input, SessionContext};
use buddy_memory::{HistoryMessage, MemoryContext, MemoryEvent, TaskState};
use buddy_personality::{
    phrase_clarification, phrase_tool_result, style_response, ClarificationAsk,
    PersonalityProfile,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tracing::{error, info, instrument};

use buddy_database::{DbError, SPARK_NUDGE_COOLDOWN_MS, SPARK_STALE_AGE_MS};

use crate::memory_api::{self, apply_plan_memory_side_effects};
use crate::memory_extraction::BrainMemoryContext;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub intent: String,
    pub tool: Option<String>,
    pub tool_input: Option<String>,
    pub reasoning: String,
    pub response: Option<String>,
    #[serde(default)]
    pub task_state: Option<TaskState>,
    #[serde(default)]
    pub preference_detected: Option<PreferenceDetected>,
    #[serde(default)]
    pub decision_detected: Option<DecisionDetected>,
    #[serde(default)]
    pub mode_hint: Option<String>,
    /// Brain-owned: `"passthrough"` skips `/chat/respond`; `"llm"` (default) narrates.
    #[serde(default)]
    pub respond_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferenceDetected {
    pub key: String,
    pub value: String,
    pub confidence: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionDetected {
    pub decision: String,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlanRequest {
    message: String,
    history: Vec<HistoryMessage>,
    memory: BrainMemoryContext,
    available_tools: String,
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
) -> Result<(), String> {
    info!(text = %text, "user request");

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
    let plan: Plan = client
        .post(format!("{}/chat/plan", state.brain_url()))
        .json(&PlanRequest {
            message: text.clone(),
            history: history.clone(),
            memory: memory.clone(),
            available_tools: state.tool_catalog_text().to_string(),
        })
        .send()
        .await
        .map_err(|e| format!("brain plan request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("brain plan parse failed: {e}"))?;

    info!(intent = %plan.intent, tool = ?plan.tool, "brain plan received");

    apply_plan_memory_side_effects(
        &state.memory,
        &ctx,
        &plan.reasoning,
        plan.task_state.as_ref(),
        plan.preference_detected.as_ref().map(|p| {
            (
                p.key.clone(),
                p.value.clone(),
                p.confidence,
                p.source.clone(),
            )
        }),
        plan.decision_detected
            .as_ref()
            .map(|d| (d.decision.clone(), d.reason.clone())),
    );

    let assistant_content = if plan.intent == "tool_use" {
        if let Some(tool_name) = &plan.tool {
            dispatch_tool(
                &app,
                state,
                &client,
                &ctx,
                &conversation_id,
                &text,
                &history,
                &memory,
                &personality,
                tool_name,
                plan.tool_input.as_deref().unwrap_or("{}"),
                plan.respond_mode.as_deref(),
            )
            .await?
        } else {
            let content = style_response(
                &personality,
                &plan
                    .response
                    .unwrap_or_else(|| "I couldn't determine the tool to use.".to_string()),
            );
            let _ = app.emit("chat-chunk", &content);
            content
        }
    } else {
        state.memory.clear_pending_clarification(&conversation_id);
        let mut content = plan.response.unwrap_or_default();
        if content.is_empty() {
            content = "I'm here to help.".to_string();
        }
        content = style_response(&personality, &content);
        let _ = app.emit("chat-chunk", &content);
        content
    };

    let assistant_metadata = serde_json::json!({
        "intent": plan.intent,
        "tool": plan.tool,
        "tool_input": plan.tool_input,
        "reasoning": plan.reasoning,
        "task_state": plan.task_state,
        "mode_hint": plan.mode_hint,
        "respond_mode": plan.respond_mode,
    })
    .to_string();

    state
        .db
        .add_message_with_metadata(
            &conversation_id,
            "assistant",
            &assistant_content,
            Some(&assistant_metadata),
        )
        .map_err(|e| e.to_string())?;

    let _ = state.memory.store_event(
        &ctx,
        MemoryEvent::MessageAdded {
            role: "assistant".into(),
            content: assistant_content.clone(),
        },
    );

    if plan.task_state == Some(TaskState::Completed) {
        let event_result = state.memory.store_event(
            &ctx,
            MemoryEvent::TaskCompleted {
                outcome: assistant_content.clone(),
            },
        )?;
        let mut all = history.clone();
        all.push(HistoryMessage {
            role: "user".into(),
            content: text.clone(),
        });
        all.push(HistoryMessage {
            role: "assistant".into(),
            content: assistant_content.clone(),
        });
        state
            .memory
            .finish_task(
                state,
                &conversation_id,
                &assistant_content,
                all,
                event_result.followups,
            )
            .await;
    }

    let _ = app.emit("chat-done", ());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn dispatch_tool(
    app: &AppHandle,
    state: &AppState,
    client: &reqwest::Client,
    ctx: &MemoryContext,
    conversation_id: &str,
    text: &str,
    history: &[HistoryMessage],
    memory: &BrainMemoryContext,
    personality: &PersonalityProfile,
    tool_name: &str,
    raw_input: &str,
    respond_mode: Option<&str>,
) -> Result<String, String> {
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
            missing_labels,
            context_hint,
        } => {
            state.memory.set_pending_clarification(
                conversation_id,
                PendingClarification {
                    tool: tool_name.to_string(),
                    tool_input: partial,
                    missing_labels: missing_labels.clone(),
                    conversation_id: conversation_id.to_string(),
                },
            );
            let question = phrase_clarification(
                personality,
                &ClarificationAsk {
                    field_labels: missing_labels,
                    context_hint,
                },
            );
            let content = style_response(personality, &question);
            let _ = app.emit("chat-chunk", &content);
            Ok(content)
        }
        ClarifyResult::Ready {
            tool_input: ready_input,
        } => {
            state.memory.clear_pending_clarification(conversation_id);
            let start = Instant::now();
            match run_tool_with_tracking(state, app, ctx, tool_name, &ready_input, start) {
                Ok(output) => {
                    info!(tool = %tool_name, respond_mode = ?respond_mode, "tool executed");
                    // Brain-owned: passthrough skips second MLX call. Buddy only routes.
                    // Personality turns JSON tool payloads into chat-ready phrasing.
                    if respond_mode == Some("passthrough") {
                        let phrased = phrase_tool_result(tool_name, &output);
                        let content = style_response(personality, &phrased);
                        let _ = app.emit("chat-chunk", &content);
                        return Ok(content);
                    }
                    let mut assistant_content = String::new();
                    let mut stream = client
                        .post(format!("{}/chat/respond", state.brain_url()))
                        .json(&RespondRequest {
                            message: text.to_string(),
                            history: history.to_vec(),
                            memory: memory.clone(),
                            tool_name: Some(tool_name.to_string()),
                            tool_result: Some(output),
                        })
                        .send()
                        .await
                        .map_err(|e| format!("brain respond request failed: {e}"))?
                        .bytes_stream();

                    use futures_util::StreamExt;
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                assistant_content.push_str(&String::from_utf8_lossy(&bytes));
                            }
                            Err(e) => {
                                error!(error = %e, "stream error");
                                break;
                            }
                        }
                    }
                    assistant_content = style_response(personality, &assistant_content);
                    let _ = app.emit("chat-chunk", &assistant_content);
                    Ok(assistant_content)
                }
                Err(err_msg) => {
                    let content =
                        style_response(personality, &format!("Tool execution failed: {err_msg}"));
                    let _ = app.emit("chat-chunk", &content);
                    Ok(content)
                }
            }
        }
    }
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

fn run_tool_with_tracking(
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
                    let _ = app.emit("calendar-updated", ());
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
