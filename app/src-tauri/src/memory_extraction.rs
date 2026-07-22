use buddy_memory::{HistoryMessage, MemoryContext, MemoryEvent, MergedContext, CONTEXT_LIMIT_THRESHOLD};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use buddy_database::Spark;

use crate::intelligence_hooks::index_saved_sync;
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
    pub workspace: Option<String>,
    pub learned_patterns: Option<String>,
    pub stale_sparks: Option<String>,
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
            workspace: payload.workspace,
            learned_patterns: payload.learned_patterns,
            stale_sparks: None,
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

pub fn spark_memory_context(spark: &Spark, state: &AppState) -> MemoryContext {
    if let Some(conv_id) = &spark.source_conversation_id {
        memory_context_for(conv_id, state)
    } else {
        MemoryContext {
            workspace_path: state.project_root.clone(),
            conversation_id: None,
            task_id: None,
        }
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
        MemoryEvent::ConversationDeleted { .. } => "conversation_archive",
        MemoryEvent::SparkDeleted { .. } => "spark_archive",
        _ => return Ok(()),
    };

    let workspace_summary = state
        .memory
        .workspace_summary(ctx)
        .map_err(|e| e.to_string())?;

    let task_outcome = match event {
        MemoryEvent::TaskCompleted { outcome } => Some(outcome.clone()),
        MemoryEvent::ConversationDeleted { title, .. } => {
            Some(format!("Conversation title: {title}"))
        }
        MemoryEvent::SparkDeleted { content, tags, .. } => {
            Some(format!("Tags: {}. Idea: {content}", tags.join(", ")))
        }
        _ => None,
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let http_response = client
        .post(format!("{}/memory/extract", state.brain_url()))
        .json(&ExtractRequest {
            kind: kind.to_string(),
            workspace_summary,
            recent_messages: recent_messages.to_vec(),
            task_outcome,
        })
        .send()
        .await
        .map_err(|e| format!("memory extract request failed: {e}"))?;

    if !http_response.status().is_success() {
        let status = http_response.status();
        let body = http_response.text().await.unwrap_or_default();
        return Err(format!("memory extract returned {status}: {body}"));
    }

    let response: ExtractResponse = http_response
        .json()
        .await
        .map_err(|e| format!("memory extract parse failed: {e}"))?;

    let extraction_data = response.data.clone();

    let conversation_meta = match event {
        MemoryEvent::ConversationDeleted {
            title,
            conversation_id,
        } => Some((title.clone(), conversation_id.clone())),
        _ => None,
    };

    let spark_meta = match event {
        MemoryEvent::SparkDeleted {
            spark_id,
            content,
            tags,
        } => Some((spark_id.clone(), content.clone(), tags.clone())),
        _ => None,
    };

    let save_event = match response.kind.as_str() {
        "handover" => MemoryEvent::HandoverSaved {
            summary: response.data,
        },
        "conversation_archive" => {
            let (title, conversation_id) = match conversation_meta {
                Some(meta) => meta,
                None => {
                    warn!("conversation_archive without ConversationDeleted metadata");
                    return Ok(());
                }
            };
            let topics = response
                .data
                .get("topics")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let key_facts = response
                .data
                .get("key_facts")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let decisions = response
                .data
                .get("decisions")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            MemoryEvent::ConversationArchivedSaved {
                title,
                conversation_id,
                summary: response
                    .data
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                topics,
                key_facts,
                decisions,
            }
        }
        "spark_archive" => {
            let (spark_id, content, tags) = match spark_meta {
                Some(meta) => meta,
                None => {
                    warn!("spark_archive without SparkDeleted metadata");
                    return Ok(());
                }
            };
            let topics = response
                .data
                .get("topics")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let key_facts = response
                .data
                .get("key_facts")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            MemoryEvent::SparkArchivedSaved {
                spark_id,
                content,
                tags,
                summary: response
                    .data
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                topics,
                key_facts,
            }
        }
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

    let result = state
        .memory_manager
        .handle_event(ctx, save_event)
        .map_err(|e| e.to_string())?;

    index_saved_sync(state, ctx, &result.saved).await;

    state.memory.on_extraction_saved(ctx, &extraction_data).await;

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

pub async fn archive_conversation_to_memory(
    state: &AppState,
    ctx: &MemoryContext,
    title: &str,
    conversation_id: &str,
    recent_messages: &[HistoryMessage],
) -> Result<(), String> {
    let event = MemoryEvent::ConversationDeleted {
        title: title.to_string(),
        conversation_id: conversation_id.to_string(),
    };
    run_memory_extraction(state, ctx, &event, recent_messages).await
}

pub async fn archive_spark_to_memory(
    state: &AppState,
    ctx: &MemoryContext,
    spark: &Spark,
) -> Result<(), String> {
    let event = MemoryEvent::SparkDeleted {
        spark_id: spark.id.clone(),
        content: spark.content.clone(),
        tags: spark.tags.clone(),
    };
    run_memory_extraction(state, ctx, &event, &[]).await
}

pub async fn save_fallback_spark_archive(
    state: &AppState,
    ctx: &MemoryContext,
    spark: &Spark,
) -> Result<(), String> {
    let tags = spark.tags.join(", ");
    let summary = if spark.content.len() > 500 {
        format!("{}...", &spark.content[..500])
    } else {
        spark.content.clone()
    };

    let event = MemoryEvent::SparkArchivedSaved {
        spark_id: spark.id.clone(),
        content: spark.content.clone(),
        tags: spark.tags.clone(),
        summary: format!("Deleted spark [{tags}]: {summary}"),
        topics: vec![],
        key_facts: vec![],
    };

    let result = state
        .memory_manager
        .handle_event(ctx, event)
        .map_err(|e| e.to_string())?;

    index_saved_sync(state, ctx, &result.saved).await;
    info!("fallback spark archive saved");
    Ok(())
}

pub async fn save_fallback_conversation_archive(
    state: &AppState,
    ctx: &MemoryContext,
    title: &str,
    conversation_id: &str,
    recent_messages: &[HistoryMessage],
) -> Result<(), String> {
    let mut transcript = String::new();
    for msg in recent_messages {
        transcript.push_str(&format!("{}: {}\n", msg.role, msg.content));
        if transcript.len() > 2000 {
            transcript.truncate(2000);
            transcript.push_str("...");
            break;
        }
    }
    let summary = if transcript.is_empty() {
        format!("Archived chat: {title}")
    } else {
        format!("Archived chat \"{title}\":\n{transcript}")
    };

    let event = MemoryEvent::ConversationArchivedSaved {
        title: title.to_string(),
        conversation_id: conversation_id.to_string(),
        summary: summary.clone(),
        topics: vec![],
        key_facts: vec![],
        decisions: vec![],
    };

    let result = state
        .memory_manager
        .handle_event(ctx, event)
        .map_err(|e| e.to_string())?;

    index_saved_sync(state, ctx, &result.saved).await;
    info!("fallback conversation archive saved");
    Ok(())
}
