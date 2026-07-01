use buddy_memory::{HistoryMessage, MemoryContext, MemoryEvent, MergedContext, CONTEXT_LIMIT_THRESHOLD};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrainMemoryContext {
    pub handover: Option<String>,
    pub working: Option<String>,
    pub project: Option<String>,
    pub preferences: Option<String>,
    pub decisions: Option<String>,
    pub errors: Option<String>,
    pub tools: Option<String>,
    pub reflections: Option<String>,
}

impl From<&MergedContext> for BrainMemoryContext {
    fn from(ctx: &MergedContext) -> Self {
        let payload = buddy_memory::MemoryContextPayload::from(ctx);
        Self {
            handover: payload.handover,
            working: payload.working,
            project: payload.project,
            preferences: payload.preferences,
            decisions: payload.decisions,
            errors: payload.errors,
            tools: payload.tools,
            reflections: payload.reflections,
        }
    }
}

#[derive(Debug, Serialize)]
struct ExtractRequest {
    kind: String,
    workspace_summary: String,
    recent_messages: Vec<HistoryMessage>,
    task_outcome: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExtractResponse {
    kind: String,
    data: serde_json::Value,
}

pub fn memory_context_for(conversation_id: &str, state: &AppState) -> MemoryContext {
    MemoryContext {
        workspace_path: state.project_root.clone(),
        conversation_id: Some(conversation_id.to_string()),
        task_id: None,
    }
}

pub async fn run_memory_extraction(
    state: &AppState,
    ctx: &MemoryContext,
    event: &MemoryEvent,
    recent_messages: &[HistoryMessage],
) -> Result<(), String> {
    let kind = match event {
        MemoryEvent::TaskCompleted { .. } => "reflection",
        MemoryEvent::ProjectChanged { .. } => "project",
        MemoryEvent::HandoverRequested => "handover",
        MemoryEvent::SessionEnding => "handover",
        MemoryEvent::ContextLimitApproaching { .. } => "handover",
        _ => return Ok(()),
    };

    let workspace_summary = state
        .memory_manager
        .workspace_summary(ctx)
        .map_err(|e| e.to_string())?;

    let task_outcome = match event {
        MemoryEvent::TaskCompleted { outcome } => Some(outcome.clone()),
        _ => None,
    };

    let client = reqwest::Client::new();
    let response: ExtractResponse = client
        .post(format!("{}/memory/extract", state.brain_url()))
        .json(&ExtractRequest {
            kind: kind.to_string(),
            workspace_summary,
            recent_messages: recent_messages.to_vec(),
            task_outcome,
        })
        .send()
        .await
        .map_err(|e| format!("memory extract request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("memory extract parse failed: {e}"))?;

    let save_event = match response.kind.as_str() {
        "handover" => MemoryEvent::HandoverSaved {
            summary: response.data,
        },
        "reflection" => MemoryEvent::ReflectionSaved {
            attempted: response
                .data
                .get("attempted")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            successful: response
                .data
                .get("successful")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            improvements: response
                .data
                .get("improvements")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            lessons: response
                .data
                .get("lessons")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
        "project" => MemoryEvent::ProjectSaved {
            section: response
                .data
                .get("section")
                .and_then(|v| v.as_str())
                .unwrap_or("general")
                .to_string(),
            content: response
                .data
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        },
        _ => {
            warn!(kind = %response.kind, "unknown extraction kind");
            return Ok(());
        }
    };

    state
        .memory_manager
        .handle_event(ctx, save_event)
        .map_err(|e| e.to_string())?;

    info!(kind = %kind, "memory extraction saved");
    Ok(())
}

pub async fn process_memory_followups(
    state: &AppState,
    ctx: &MemoryContext,
    followups: Vec<MemoryEvent>,
    recent_messages: &[HistoryMessage],
) {
    for event in followups {
        if event.needs_extraction() {
            if let Err(e) =
                run_memory_extraction(state, ctx, &event, recent_messages).await
            {
                warn!(error = %e, "memory extraction failed");
            }
        }
    }
}

pub async fn maybe_handover_on_context_limit(
    state: &AppState,
    ctx: &MemoryContext,
    merged: &MergedContext,
    recent_messages: &[HistoryMessage],
) {
    if merged.estimated_tokens >= CONTEXT_LIMIT_THRESHOLD {
        let event = MemoryEvent::ContextLimitApproaching {
            estimated_tokens: merged.estimated_tokens,
        };
        if let Err(e) = run_memory_extraction(state, ctx, &event, recent_messages).await {
            warn!(error = %e, "context limit handover failed");
        }
        let _ = state
            .memory_manager
            .compress_old_handovers(ctx, 3);
    }
}

pub async fn session_end_handover(state: &AppState, ctx: &MemoryContext) {
    let event = MemoryEvent::SessionEnding;
    if let Err(e) = run_memory_extraction(state, ctx, &event, &[]).await {
        warn!(error = %e, "session end handover failed");
    }
    let _ = state.memory_manager.prune_expired_working(ctx);
}
