use std::collections::HashMap;

use buddy_memory::{
    estimate_tokens, ContextSection, HistoryMessage, MemoryContext, MemoryKind, MemoryManager,
    MergedContext, RetrieveQuery, DEFAULT_CONVERSATION_WINDOW,
};
use tracing::debug;

use crate::knowledge_graph::KnowledgeGraph;
use crate::learning::LearningEngine;
use crate::semantic::{embed_with_fallback, ScoredMemory, SearchQuery, SemanticSearch};
use crate::workspace::WorkspaceIntel;

const SEMANTIC_LIMIT: usize = 20;
const MIN_SIMILARITY: f32 = 0.55;

pub struct ContextBuilder;

impl ContextBuilder {
    pub async fn build<S: SemanticSearch + ?Sized>(
        memory: &MemoryManager,
        search: &S,
        kg: &KnowledgeGraph,
        learning: &LearningEngine,
        workspace_intel: &WorkspaceIntel,
        ctx: &MemoryContext,
        query: &str,
        token_budget: usize,
    ) -> Result<MergedContext, crate::error::IntelligenceError> {
        let workspace = ctx.workspace_path.display().to_string();
        let retrieve_query = RetrieveQuery {
            workspace_path: ctx.workspace_path.clone(),
            conversation_id: ctx.conversation_id.clone(),
            task_id: ctx.task_id.clone(),
            keywords: Some(query.to_string()),
            limit: None,
        };

        let mut handover: Option<String> = None;
        let mut sections: Vec<ContextSection> = Vec::new();
        let mut total_tokens = 0;

        // Pinned: workspace profile
        if let Ok(profile_text) = workspace_intel.summary(&workspace) {
            if !profile_text.is_empty() {
                let tokens = estimate_tokens(&profile_text);
                total_tokens += tokens;
                sections.push(ContextSection {
                    kind: MemoryKind::Project,
                    content: format!("Workspace overview:\n{profile_text}"),
                });
            }
        }

        // Pinned: latest handover
        if let Ok(h) = memory.summarize_kind(MemoryKind::Handover, &retrieve_query) {
            if !h.is_empty() {
                let tokens = estimate_tokens(&h);
                handover = Some(h);
                total_tokens += tokens;
            }
        }

        // Pinned: active working memory
        if let Ok(w) = memory.summarize_kind(MemoryKind::Working, &retrieve_query) {
            if !w.is_empty() {
                let tokens = estimate_tokens(&w);
                if total_tokens + tokens <= token_budget {
                    total_tokens += tokens;
                    sections.push(ContextSection {
                        kind: MemoryKind::Working,
                        content: w,
                    });
                }
            }
        }

        // Pinned: high-confidence learned patterns
        if let Ok(patterns) = learning.format_for_context(&workspace) {
            if !patterns.is_empty() {
                let tokens = estimate_tokens(&patterns);
                if total_tokens + tokens <= token_budget {
                    total_tokens += tokens;
                    sections.push(ContextSection {
                        kind: MemoryKind::Reflection,
                        content: patterns,
                    });
                }
            }
        }

        // Semantic search for query-relevant memories
        let query_embedding = if query.trim().is_empty() {
            Vec::new()
        } else {
            embed_with_fallback(search, query).await
        };

        let mut kind_content: HashMap<MemoryKind, Vec<String>> = HashMap::new();

        if !query_embedding.is_empty() {
            let scored = search
                .search(&SearchQuery {
                    workspace_path: workspace.clone(),
                    query_embedding,
                    kinds: None,
                    limit: SEMANTIC_LIMIT,
                    min_similarity: MIN_SIMILARITY,
                })
                .await?;

            for hit in &scored {
                let line = format_memory_hit(hit);
                kind_content.entry(hit.kind).or_default().push(line);
            }

            // Knowledge graph 1-hop expansion
            if let Ok(related) = kg.related_context(&workspace, &scored) {
                if !related.is_empty() {
                    kind_content
                        .entry(MemoryKind::Project)
                        .or_default()
                        .push(format!("Related knowledge:\n{related}"));
                }
            }
        }

        // Fill remaining budget with ranked semantic sections
        let ranked_kinds = [
            MemoryKind::Decision,
            MemoryKind::Reflection,
            MemoryKind::Error,
            MemoryKind::Project,
            MemoryKind::Preference,
            MemoryKind::Tool,
        ];

        for kind in ranked_kinds {
            let lines = kind_content.remove(&kind);
            let content = if let Some(lines) = lines {
                format_section(kind, &lines)
            } else if query.trim().is_empty() {
                memory.summarize_kind(kind, &retrieve_query)?
            } else {
                String::new()
            };

            if content.is_empty() {
                continue;
            }
            let tokens = estimate_tokens(&content);
            if total_tokens + tokens > token_budget {
                debug!(kind = ?kind, "skipping section due to token budget");
                continue;
            }
            total_tokens += tokens;
            sections.push(ContextSection {
                kind,
                content,
            });
        }

        sections.sort_by_key(|s| s.kind.retrieval_priority());

        // Conversation: semantic matches + last 3 for continuity
        let conversation_messages =
            build_conversation_context(memory, search, ctx, query, &retrieve_query).await?;

        total_tokens += conversation_messages
            .iter()
            .map(|m| estimate_tokens(&m.content))
            .sum::<usize>();

        Ok(MergedContext {
            handover,
            sections,
            conversation_messages,
            estimated_tokens: total_tokens,
        })
    }
}

async fn build_conversation_context<S: SemanticSearch + ?Sized>(
    memory: &MemoryManager,
    search: &S,
    ctx: &MemoryContext,
    query: &str,
    retrieve_query: &RetrieveQuery,
) -> Result<Vec<HistoryMessage>, crate::error::IntelligenceError> {
    let _ = ctx;
    let conv_module = memory.module(MemoryKind::Conversation)?;
    let conv_records = conv_module.retrieve(retrieve_query)?;
    let all_messages: Vec<HistoryMessage> = conv_records
        .iter()
        .filter_map(|r| {
            Some(HistoryMessage {
                role: r.payload.get("role")?.as_str()?.to_string(),
                content: r.payload.get("content")?.as_str()?.to_string(),
            })
        })
        .collect();

    if query.trim().is_empty() || all_messages.len() <= 5 {
        let start = all_messages.len().saturating_sub(DEFAULT_CONVERSATION_WINDOW.min(10));
        return Ok(all_messages[start..].to_vec());
    }

    let query_embedding = embed_with_fallback(search, query).await;
    if query_embedding.is_empty() {
        let start = all_messages.len().saturating_sub(DEFAULT_CONVERSATION_WINDOW.min(10));
        return Ok(all_messages[start..].to_vec());
    }

    let mut scored_msgs: Vec<(f32, HistoryMessage)> = Vec::new();

    for msg in &all_messages {
        if msg.content.len() < 10 {
            continue;
        }
        let emb = embed_with_fallback(search, &msg.content).await;
        if emb.len() == query_embedding.len() {
            let sim = crate::semantic::cosine_similarity(&query_embedding, &emb);
            if sim >= MIN_SIMILARITY {
                scored_msgs.push((sim, msg.clone()));
            }
        }
    }

    scored_msgs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored_msgs.truncate(5);

    let mut result: Vec<HistoryMessage> = scored_msgs.into_iter().map(|(_, m)| m).collect();

    // Always include last 3 messages for continuity
    let tail_start = all_messages.len().saturating_sub(3);
    for msg in &all_messages[tail_start..] {
        if !result.iter().any(|m| m.content == msg.content && m.role == msg.role) {
            result.push(msg.clone());
        }
    }

    if result.is_empty() {
        let start = all_messages.len().saturating_sub(10);
        return Ok(all_messages[start..].to_vec());
    }

    Ok(result)
}

fn format_memory_hit(hit: &ScoredMemory) -> String {
    if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&hit.payload) {
        match hit.kind {
            MemoryKind::Decision => {
                let d = payload.get("decision").and_then(|v| v.as_str()).unwrap_or("");
                let r = payload.get("reason").and_then(|v| v.as_str()).unwrap_or("");
                return format!("- {d}: {r}");
            }
            MemoryKind::Preference => {
                let k = payload.get("key").and_then(|v| v.as_str()).unwrap_or("");
                let v = payload.get("value").and_then(|v| v.as_str()).unwrap_or("");
                return format!("- {k}: {v}");
            }
            MemoryKind::Reflection => {
                let l = payload.get("lessons").and_then(|v| v.as_str()).unwrap_or("");
                return format!("- {l}");
            }
            MemoryKind::Error => {
                let e = payload.get("error").and_then(|v| v.as_str()).unwrap_or("");
                return format!("- {e}");
            }
            MemoryKind::Project => {
                let s = payload.get("section").and_then(|v| v.as_str()).unwrap_or("");
                let c = payload.get("content").and_then(|v| v.as_str()).unwrap_or("");
                return format!("- [{s}] {c}");
            }
            MemoryKind::Tool => {
                let t = payload.get("tool").and_then(|v| v.as_str()).unwrap_or("");
                return format!("- {t}");
            }
            _ => {}
        }
    }
    format!("- {}", hit.search_text)
}

fn format_section(kind: MemoryKind, lines: &[String]) -> String {
    let header = match kind {
        MemoryKind::Decision => "Architectural decisions",
        MemoryKind::Reflection => "Reflections",
        MemoryKind::Error => "Known errors",
        MemoryKind::Project => "Project knowledge",
        MemoryKind::Preference => "User preferences",
        MemoryKind::Tool => "Recent tools",
        _ => "Memory",
    };
    format!("{header}:\n{}", lines.join("\n"))
}
