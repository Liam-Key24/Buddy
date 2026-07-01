use std::time::Instant;

use buddy_memory::{HistoryMessage, MemoryContext, MemoryEvent, TaskState};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tracing::{error, info, instrument};

use crate::intelligence_hooks::spawn_index_saved;
use crate::memory_extraction::{
    maybe_handover_on_context_limit, memory_context_for, process_memory_followups,
    BrainMemoryContext,
};
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
}

#[derive(Debug, Serialize, Deserialize)]
struct RespondRequest {
    message: String,
    history: Vec<HistoryMessage>,
    memory: BrainMemoryContext,
    tool_name: Option<String>,
    tool_result: Option<String>,
}

fn emit_task_events(state: &AppState, ctx: &MemoryContext, plan: &Plan) -> Result<(), String> {
    let mut all_saved = Vec::new();

    if let Some(task_state) = &plan.task_state {
        match task_state {
            TaskState::Started => {
                let result = state.memory_manager.handle_event(
                    ctx,
                    MemoryEvent::TaskStarted {
                        objective: plan.reasoning.clone(),
                        plan: Some(plan.reasoning.clone()),
                        files: vec![],
                    },
                ).map_err(|e| e.to_string())?;
                all_saved.extend(result.saved);
            }
            TaskState::Updated => {
                let result = state.memory_manager.handle_event(
                    ctx,
                    MemoryEvent::TaskUpdated {
                        objective: None,
                        plan: Some(plan.reasoning.clone()),
                        files: None,
                        notes: None,
                    },
                ).map_err(|e| e.to_string())?;
                all_saved.extend(result.saved);
            }
            TaskState::Completed => {}
        }
    }

    if let Some(pref) = &plan.preference_detected {
        let result = state.memory_manager.handle_event(
            ctx,
            MemoryEvent::PreferenceDetected {
                key: pref.key.clone(),
                value: pref.value.clone(),
                confidence: pref.confidence,
                source: pref.source.clone(),
            },
        );
        if let Ok(r) = result {
            all_saved.extend(r.saved);
        }
    }

    if let Some(dec) = &plan.decision_detected {
        let result = state
            .memory_manager
            .handle_event(
                ctx,
                MemoryEvent::DecisionRecorded {
                    decision: dec.decision.clone(),
                    reason: dec.reason.clone(),
                },
            )
            .map_err(|e| e.to_string())?;
        all_saved.extend(result.saved);
    }

    spawn_index_saved(state, ctx, &all_saved);
    Ok(())
}

async fn handle_handover_command(
    app: &AppHandle,
    state: &AppState,
    conversation_id: &str,
    ctx: &MemoryContext,
    recent_messages: &[HistoryMessage],
) -> Result<(), String> {
    let event = MemoryEvent::HandoverRequested;
    crate::memory_extraction::run_memory_extraction(state, ctx, &event, recent_messages).await?;

    let merged = state
        .intelligence
        .build_context(ctx, "")
        .await
        .map_err(|e| e.to_string())?;

    let handover_text = merged
        .handover
        .unwrap_or_else(|| "Handover generated and saved.".to_string());

    state
        .db
        .add_message(conversation_id, "assistant", &handover_text)
        .map_err(|e| e.to_string())?;

    let _ = app.emit("chat-chunk", &handover_text);
    let _ = app.emit("chat-done", ());
    Ok(())
}

#[instrument(skip(state, app), fields(conversation_id = %conversation_id))]
pub async fn send_message(
    app: AppHandle,
    state: &AppState,
    conversation_id: String,
    text: String,
) -> Result<(), String> {
    info!(text = %text, "user request");

    let ctx = memory_context_for(&conversation_id, state);

    let merged = state
        .intelligence
        .build_context(&ctx, &text)
        .await
        .map_err(|e| e.to_string())?;

    let trimmed_history = merged.conversation_messages.clone();

    maybe_handover_on_context_limit(state, &ctx, &merged, &trimmed_history).await;

    if text.trim().starts_with("/maintain") {
        state
            .db
            .add_message(&conversation_id, "user", &text)
            .map_err(|e| e.to_string())?;
        let report = state
            .intelligence
            .run_maintenance(&ctx)
            .await
            .map_err(|e| e.to_string())?;
        let msg = format!(
            "Maintenance complete: merged {}, archived {}, conflicts {}.",
            report.merged_duplicates, report.archived, report.conflicts_detected
        );
        state
            .db
            .add_message(&conversation_id, "assistant", &msg)
            .map_err(|e| e.to_string())?;
        let _ = app.emit("chat-chunk", &msg);
        let _ = app.emit("chat-done", ());
        return Ok(());
    }

    if text.trim().starts_with("/handover") {
        state
            .db
            .add_message(&conversation_id, "user", &text)
            .map_err(|e| e.to_string())?;
        let _ = state.memory_manager.handle_event(
            &ctx,
            MemoryEvent::MessageAdded {
                role: "user".into(),
                content: text.clone(),
            },
        );
        return handle_handover_command(&app, state, &conversation_id, &ctx, &trimmed_history).await;
    }

    state
        .db
        .add_message(&conversation_id, "user", &text)
        .map_err(|e| e.to_string())?;

    let _ = state.memory_manager.handle_event(
        &ctx,
        MemoryEvent::MessageAdded {
            role: "user".into(),
            content: text.clone(),
        },
    );

    let messages = state.db.get_messages(&conversation_id).map_err(|e| e.to_string())?;
    if messages.len() == 1 {
        let title: String = text.chars().take(40).collect();
        let _ = state.db.update_conversation_title(&conversation_id, &title);
    }

    let memory = BrainMemoryContext::from(&merged);
    let client = reqwest::Client::new();

    let plan: Plan = client
        .post(format!("{}/chat/plan", state.brain_url()))
        .json(&PlanRequest {
            message: text.clone(),
            history: trimmed_history.clone(),
            memory: memory.clone(),
        })
        .send()
        .await
        .map_err(|e| format!("brain plan request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("brain plan parse failed: {e}"))?;

    info!(
        intent = %plan.intent,
        tool = ?plan.tool,
        "brain plan received"
    );

    emit_task_events(state, &ctx, &plan)?;

    let mut assistant_content = String::new();

    if plan.intent == "tool_use" {
        if let (Some(tool_name), Some(tool_input)) = (&plan.tool, &plan.tool_input) {
            let start = Instant::now();
            let tool_result = match state.task_runner.run(tool_name, tool_input) {
                Ok(run_result) => {
                    let output = run_result.output.clone();
                    let duration_ms = start.elapsed().as_millis() as u64;
                    let event_result = state.memory_manager.handle_event(
                        &ctx,
                        MemoryEvent::ToolExecuted {
                            tool: tool_name.clone(),
                            params: tool_input.clone(),
                            result: output.clone(),
                            duration_ms,
                            success: true,
                        },
                    );
                    if let Ok(r) = event_result {
                        spawn_index_saved(state, &ctx, &r.saved);
                    }
                    Ok(output)
                }
                Err(e) => {
                    let duration_ms = start.elapsed().as_millis() as u64;
                    let err_msg = e.to_string();
                    let exec_result = state.memory_manager.handle_event(
                        &ctx,
                        MemoryEvent::ToolExecuted {
                            tool: tool_name.clone(),
                            params: tool_input.clone(),
                            result: err_msg.clone(),
                            duration_ms,
                            success: false,
                        },
                    );
                    if let Ok(r) = exec_result {
                        spawn_index_saved(state, &ctx, &r.saved);
                    }
                    let fail_result = state.memory_manager.handle_event(
                        &ctx,
                        MemoryEvent::ToolFailed {
                            error: err_msg.clone(),
                            cause: err_msg.clone(),
                            resolution: None,
                        },
                    );
                    if let Ok(r) = fail_result {
                        spawn_index_saved(state, &ctx, &r.saved);
                    }
                    Err(err_msg)
                }
            };

            match tool_result {
                Ok(output) => {
                    info!(tool = %tool_name, output = %output, "tool executed");

                    let mut stream = client
                        .post(format!("{}/chat/respond", state.brain_url()))
                        .json(&RespondRequest {
                            message: text.clone(),
                            history: trimmed_history.clone(),
                            memory: memory.clone(),
                            tool_name: Some(tool_name.clone()),
                            tool_result: Some(output.clone()),
                        })
                        .send()
                        .await
                        .map_err(|e| format!("brain respond request failed: {e}"))?
                        .bytes_stream();

                    use futures_util::StreamExt;
                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                let piece = String::from_utf8_lossy(&bytes).to_string();
                                assistant_content.push_str(&piece);
                                let _ = app.emit("chat-chunk", &piece);
                            }
                            Err(e) => {
                                error!(error = %e, "stream error");
                                break;
                            }
                        }
                    }
                }
                Err(err_msg) => {
                    assistant_content = format!("Tool execution failed: {err_msg}");
                    let _ = app.emit("chat-chunk", &assistant_content);
                }
            }
        } else {
            assistant_content = plan
                .response
                .unwrap_or_else(|| "I couldn't determine the tool to use.".to_string());
            let _ = app.emit("chat-chunk", &assistant_content);
        }
    } else {
        assistant_content = plan.response.unwrap_or_default();
        if assistant_content.is_empty() {
            assistant_content = "I'm here to help.".to_string();
        }
        let _ = app.emit("chat-chunk", &assistant_content);
    }

    let assistant_metadata = serde_json::json!({
        "intent": plan.intent,
        "tool": plan.tool,
        "tool_input": plan.tool_input,
        "reasoning": plan.reasoning,
        "task_state": plan.task_state,
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

    let _ = state.memory_manager.handle_event(
        &ctx,
        MemoryEvent::MessageAdded {
            role: "assistant".into(),
            content: assistant_content.clone(),
        },
    );

    if plan.task_state == Some(TaskState::Completed) {
        let event_result = state
            .memory_manager
            .handle_event(
                &ctx,
                MemoryEvent::TaskCompleted {
                    outcome: assistant_content.clone(),
                },
            )
            .map_err(|e| e.to_string())?;

        let _ = state
            .intelligence
            .on_task_complete(&ctx, &assistant_content)
            .await;

        let mut all_messages = trimmed_history.clone();
        all_messages.push(HistoryMessage {
            role: "user".into(),
            content: text.clone(),
        });
        all_messages.push(HistoryMessage {
            role: "assistant".into(),
            content: assistant_content.clone(),
        });

        process_memory_followups(state, &ctx, event_result.followups, &all_messages).await;

        state.intelligence.spawn_maintenance(ctx.clone()).await;
    }

    let _ = app.emit("chat-done", ());
    Ok(())
}
