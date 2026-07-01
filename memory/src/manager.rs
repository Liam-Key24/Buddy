use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use buddy_database::Database;
use tracing::{debug, info, instrument};
use uuid::Uuid;

use crate::error::MemoryError;
use crate::events::MemoryEvent;
use crate::memory_trait::Memory;
use crate::modules::{
    ConversationMemory, DecisionMemory, ErrorMemory, HandoverMemory, PreferenceMemory,
    ProjectMemory, ReflectionMemory, ToolMemory, WorkingMemory,
};
use crate::storage::SqliteStorageBackend;
use crate::types::{
    estimate_tokens, ContextSection, HistoryMessage, MemoryContext, MemoryKind, MemoryRecord,
    MergedContext, RetrieveQuery, DEFAULT_TOKEN_BUDGET,
};

pub struct MemoryManager {
    modules: HashMap<MemoryKind, Arc<dyn Memory>>,
    active_task_ids: Mutex<HashMap<String, String>>,
    token_budget: usize,
}

impl MemoryManager {
    pub fn new(db: Arc<Database>) -> Self {
        let storage = Arc::new(SqliteStorageBackend::new(db.clone()));
        let mut modules: HashMap<MemoryKind, Arc<dyn Memory>> = HashMap::new();

        modules.insert(
            MemoryKind::Conversation,
            Arc::new(ConversationMemory::new(db)),
        );
        modules.insert(
            MemoryKind::Working,
            Arc::new(WorkingMemory::new(storage.clone())),
        );
        modules.insert(
            MemoryKind::Project,
            Arc::new(ProjectMemory::new(storage.clone())),
        );
        modules.insert(
            MemoryKind::Preference,
            Arc::new(PreferenceMemory::new(storage.clone())),
        );
        modules.insert(
            MemoryKind::Handover,
            Arc::new(HandoverMemory::new(storage.clone())),
        );
        modules.insert(
            MemoryKind::Decision,
            Arc::new(DecisionMemory::new(storage.clone())),
        );
        modules.insert(
            MemoryKind::Error,
            Arc::new(ErrorMemory::new(storage.clone())),
        );
        modules.insert(
            MemoryKind::Tool,
            Arc::new(ToolMemory::new(storage.clone())),
        );
        modules.insert(
            MemoryKind::Reflection,
            Arc::new(ReflectionMemory::new(storage.clone())),
        );

        Self {
            modules,
            active_task_ids: Mutex::new(HashMap::new()),
            token_budget: DEFAULT_TOKEN_BUDGET,
        }
    }

    fn module(&self, kind: MemoryKind) -> Result<&Arc<dyn Memory>, MemoryError> {
        self.modules
            .get(&kind)
            .ok_or_else(|| MemoryError::NotFound(format!("{kind:?}")))
    }

    fn workspace_key(ctx: &MemoryContext) -> String {
        ctx.workspace_path.display().to_string()
    }

    #[instrument(skip(self))]
    pub fn retrieve_for_prompt(
        &self,
        ctx: &MemoryContext,
        user_message: &str,
    ) -> Result<MergedContext, MemoryError> {
        let query = RetrieveQuery {
            workspace_path: ctx.workspace_path.clone(),
            conversation_id: ctx.conversation_id.clone(),
            task_id: ctx.task_id.clone(),
            keywords: Some(user_message.to_string()),
            limit: None,
        };

        let retrieval_order = [
            MemoryKind::Handover,
            MemoryKind::Working,
            MemoryKind::Conversation,
            MemoryKind::Project,
            MemoryKind::Preference,
            MemoryKind::Decision,
            MemoryKind::Error,
            MemoryKind::Tool,
            MemoryKind::Reflection,
        ];

        let mut handover: Option<String> = None;
        let mut sections = Vec::new();
        let mut total_tokens = 0;

        for kind in retrieval_order {
            if kind == MemoryKind::Conversation {
                continue;
            }
            let module = self.module(kind)?;
            let summary = module.summarize(&query)?;
            if summary.is_empty() {
                continue;
            }
            let tokens = estimate_tokens(&summary);
            if kind == MemoryKind::Handover {
                handover = Some(summary);
                total_tokens += tokens;
                continue;
            }
            if total_tokens + tokens > self.token_budget {
                debug!(kind = ?kind, "skipping section due to token budget");
                continue;
            }
            total_tokens += tokens;
            sections.push(ContextSection {
                kind,
                content: summary,
            });
        }

        sections.sort_by_key(|s| s.kind.retrieval_priority());

        let conv_module = self.module(MemoryKind::Conversation)?;
        let conv_records = conv_module.retrieve(&query)?;
        let conversation_messages: Vec<HistoryMessage> = conv_records
            .iter()
            .filter_map(|r| {
                Some(HistoryMessage {
                    role: r.payload.get("role")?.as_str()?.to_string(),
                    content: r.payload.get("content")?.as_str()?.to_string(),
                })
            })
            .collect();

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

    pub fn workspace_summary(&self, ctx: &MemoryContext) -> Result<String, MemoryError> {
        let merged = self.retrieve_for_prompt(ctx, "")?;
        let mut parts = Vec::new();
        if let Some(h) = &merged.handover {
            parts.push(format!("Handover:\n{h}"));
        }
        for section in &merged.sections {
            parts.push(format!("{:?}:\n{}", section.kind, section.content));
        }
        Ok(parts.join("\n\n"))
    }

    #[instrument(skip(self, event))]
    pub fn handle_event(
        &self,
        ctx: &MemoryContext,
        event: MemoryEvent,
    ) -> Result<Vec<MemoryEvent>, MemoryError> {
        let mut follow_up = Vec::new();
        let workspace = Self::workspace_key(ctx);

        match &event {
            MemoryEvent::MessageAdded { .. } => {
                // Conversation memory reads from the messages table directly.
            }
            MemoryEvent::TaskStarted {
                objective,
                plan,
                files,
            } => {
                let task_id = Uuid::new_v4().to_string();
                {
                    let mut tasks = self.active_task_ids.lock().unwrap();
                    tasks.insert(workspace.clone(), task_id.clone());
                }
                let module = self.module(MemoryKind::Working)?;
                module.save(
                    ctx,
                    MemoryRecord {
                        id: None,
                        kind: MemoryKind::Working,
                        payload: serde_json::json!({
                            "task_id": task_id,
                            "objective": objective,
                            "plan": plan,
                            "files": files,
                            "notes": "",
                            "status": "active",
                        }),
                        created_at: None,
                        updated_at: None,
                    },
                )?;
            }
            MemoryEvent::TaskUpdated {
                objective,
                plan,
                files,
                notes,
            } => {
                let module = self.module(MemoryKind::Working)?;
                let records = module.retrieve(&RetrieveQuery {
                    workspace_path: ctx.workspace_path.clone(),
                    conversation_id: ctx.conversation_id.clone(),
                    task_id: ctx.task_id.clone(),
                    keywords: None,
                    limit: Some(1),
                })?;
                if let Some(record) = records.first() {
                    let id = record.id.as_deref().unwrap_or("");
                    let mut payload = record.payload.clone();
                    if let Some(obj) = payload.as_object_mut() {
                        if let Some(o) = objective {
                            obj.insert("objective".into(), serde_json::json!(o));
                        }
                        if let Some(p) = plan {
                            obj.insert("plan".into(), serde_json::json!(p));
                        }
                        if let Some(f) = files {
                            obj.insert("files".into(), serde_json::json!(f));
                        }
                        if let Some(n) = notes {
                            obj.insert("notes".into(), serde_json::json!(n));
                        }
                    }
                    module.update(
                        id,
                        MemoryRecord {
                            id: Some(id.to_string()),
                            kind: MemoryKind::Working,
                            payload,
                            created_at: record.created_at,
                            updated_at: None,
                        },
                    )?;
                }
            }
            MemoryEvent::TaskCompleted { outcome } => {
                let module = self.module(MemoryKind::Working)?;
                let records = module.retrieve(&RetrieveQuery {
                    workspace_path: ctx.workspace_path.clone(),
                    conversation_id: ctx.conversation_id.clone(),
                    task_id: ctx.task_id.clone(),
                    keywords: None,
                    limit: Some(1),
                })?;
                for record in records {
                    if let Some(id) = &record.id {
                        module.delete(id)?;
                    }
                }
                {
                    let mut tasks = self.active_task_ids.lock().unwrap();
                    tasks.remove(&workspace);
                }
                follow_up.push(MemoryEvent::TaskCompleted {
                    outcome: outcome.clone(),
                });
            }
            MemoryEvent::ToolExecuted {
                tool,
                params,
                result,
                duration_ms,
                success,
            } => {
                let module = self.module(MemoryKind::Tool)?;
                module.save(
                    ctx,
                    MemoryRecord {
                        id: None,
                        kind: MemoryKind::Tool,
                        payload: serde_json::json!({
                            "tool": tool,
                            "params": params,
                            "result": result,
                            "duration_ms": duration_ms,
                            "success": success,
                        }),
                        created_at: None,
                        updated_at: None,
                    },
                )?;
            }
            MemoryEvent::ToolFailed {
                error,
                cause,
                resolution,
            } => {
                let module = self.module(MemoryKind::Error)?;
                module.save(
                    ctx,
                    MemoryRecord {
                        id: None,
                        kind: MemoryKind::Error,
                        payload: serde_json::json!({
                            "error": error,
                            "cause": cause,
                            "resolution": resolution,
                            "frequency": 1,
                        }),
                        created_at: None,
                        updated_at: None,
                    },
                )?;
            }
            MemoryEvent::DecisionRecorded { decision, reason } => {
                let module = self.module(MemoryKind::Decision)?;
                module.save(
                    ctx,
                    MemoryRecord {
                        id: None,
                        kind: MemoryKind::Decision,
                        payload: serde_json::json!({
                            "decision": decision,
                            "reason": reason,
                        }),
                        created_at: None,
                        updated_at: None,
                    },
                )?;
            }
            MemoryEvent::PreferenceDetected {
                key,
                value,
                confidence,
                source,
            } => {
                let module = self.module(MemoryKind::Preference)?;
                let _ = module.save(
                    ctx,
                    MemoryRecord {
                        id: None,
                        kind: MemoryKind::Preference,
                        payload: serde_json::json!({
                            "key": key,
                            "value": value,
                            "confidence": confidence,
                            "source": source,
                        }),
                        created_at: None,
                        updated_at: None,
                    },
                );
            }
            MemoryEvent::ProjectChanged { hint: _ } => {
                follow_up.push(event.clone());
            }
            MemoryEvent::HandoverRequested
            | MemoryEvent::SessionEnding
            | MemoryEvent::ContextLimitApproaching { .. } => {
                follow_up.push(event.clone());
            }
            MemoryEvent::HandoverSaved { summary } => {
                let module = self.module(MemoryKind::Handover)?;
                module.save(
                    ctx,
                    MemoryRecord {
                        id: None,
                        kind: MemoryKind::Handover,
                        payload: serde_json::json!({ "summary": summary }),
                        created_at: None,
                        updated_at: None,
                    },
                )?;
                info!("handover saved for workspace");
            }
            MemoryEvent::ReflectionSaved {
                attempted,
                successful,
                improvements,
                lessons,
            } => {
                let module = self.module(MemoryKind::Reflection)?;
                module.save(
                    ctx,
                    MemoryRecord {
                        id: None,
                        kind: MemoryKind::Reflection,
                        payload: serde_json::json!({
                            "attempted": attempted,
                            "successful": successful,
                            "improvements": improvements,
                            "lessons": lessons,
                        }),
                        created_at: None,
                        updated_at: None,
                    },
                )?;
            }
            MemoryEvent::ProjectSaved { section, content } => {
                let module = self.module(MemoryKind::Project)?;
                module.save(
                    ctx,
                    MemoryRecord {
                        id: None,
                        kind: MemoryKind::Project,
                        payload: serde_json::json!({
                            "section": section,
                            "content": content,
                        }),
                        created_at: None,
                        updated_at: None,
                    },
                )?;
            }
        }

        Ok(follow_up)
    }

    pub fn compress_old_handovers(&self, ctx: &MemoryContext, keep: usize) -> Result<(), MemoryError> {
        let module = self.module(MemoryKind::Handover)?;
        let records = module.retrieve(&RetrieveQuery {
            workspace_path: ctx.workspace_path.clone(),
            conversation_id: None,
            task_id: None,
            keywords: None,
            limit: Some(100),
        })?;
        for record in records.iter().skip(keep) {
            if let Some(id) = &record.id {
                module.delete(id)?;
            }
        }
        Ok(())
    }

    pub fn prune_expired_working(&self, ctx: &MemoryContext) -> Result<(), MemoryError> {
        let module = self.module(MemoryKind::Working)?;
        let records = module.retrieve(&RetrieveQuery {
            workspace_path: ctx.workspace_path.clone(),
            conversation_id: None,
            task_id: None,
            keywords: None,
            limit: Some(100),
        })?;
        for record in records {
            if let Some(id) = &record.id {
                module.delete(id)?;
            }
        }
        Ok(())
    }
}
