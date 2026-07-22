//! Memory API: Buddy's only interface to persistence and retrieval.
//! Intelligence stays an internal detail of this facade.

use std::sync::Arc;

use buddy_clarification::PendingClarification;
use buddy_database::{Database, SPARK_NUDGE_COOLDOWN_MS, SPARK_STALE_AGE_MS};
use buddy_intelligence::IntelligenceService;
use buddy_memory::{
    HistoryMessage, MemoryContext, MemoryEvent, MemoryKind, MemoryManager, MergedContext,
    SavedMemory, TaskState,
};
use tracing::warn;

use crate::memory_extraction::{
    archive_conversation_to_memory, maybe_handover_on_context_limit, process_memory_followups,
    run_memory_extraction, save_fallback_conversation_archive, BrainMemoryContext,
};
use crate::services::ProcessManager;
use crate::state::AppState;

/// Facade Buddy uses instead of talking to Intelligence directly.
#[derive(Clone)]
pub struct MemoryApi {
    pub manager: Arc<MemoryManager>,
    intelligence: Arc<IntelligenceService>,
    db: Arc<Database>,
    project_root: std::path::PathBuf,
}

impl MemoryApi {
    pub fn new(
        manager: Arc<MemoryManager>,
        intelligence: Arc<IntelligenceService>,
        db: Arc<Database>,
        project_root: std::path::PathBuf,
    ) -> Self {
        Self {
            manager,
            intelligence,
            db,
            project_root,
        }
    }

    pub fn ctx(&self, conversation_id: &str) -> MemoryContext {
        MemoryContext {
            workspace_path: self.project_root.clone(),
            conversation_id: Some(conversation_id.to_string()),
            task_id: None,
        }
    }

    /// Soft-fail context build: history-only context if Intelligence fails.
    pub async fn get_context(&self, conversation_id: &str, query: &str) -> MergedContext {
        let ctx = self.ctx(conversation_id);
        match self.intelligence.build_context(&ctx, query).await {
            Ok(merged) => merged,
            Err(e) => {
                warn!(error = %e, "memory context failed — continuing with history only");
                fallback_context(&self.db, conversation_id)
            }
        }
    }

    pub fn store_event(
        &self,
        ctx: &MemoryContext,
        event: MemoryEvent,
    ) -> Result<buddy_memory::HandleEventResult, String> {
        self.manager
            .handle_event(ctx, event)
            .map_err(|e| e.to_string())
    }

    pub async fn create_handover(
        &self,
        state: &AppState,
        conversation_id: &str,
        recent: &[HistoryMessage],
    ) -> Result<String, String> {
        let ctx = self.ctx(conversation_id);
        run_memory_extraction(state, &ctx, &MemoryEvent::HandoverRequested, recent).await?;
        let merged = self.get_context(conversation_id, "").await;
        Ok(merged
            .handover
            .unwrap_or_else(|| "Handover generated and saved.".to_string()))
    }

    pub async fn run_maintenance(&self, conversation_id: &str) -> Result<String, String> {
        let ctx = self.ctx(conversation_id);
        let report = self
            .intelligence
            .run_maintenance(&ctx)
            .await
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "Maintenance complete: merged {}, archived {}, conflicts {}.",
            report.merged_duplicates, report.archived, report.conflicts_detected
        ))
    }

    pub async fn maybe_auto_handover(
        &self,
        state: &AppState,
        conversation_id: &str,
        merged: &MergedContext,
        history: &[HistoryMessage],
    ) {
        let ctx = self.ctx(conversation_id);
        maybe_handover_on_context_limit(state, &ctx, merged, history).await;
    }

    pub fn brain_payload(&self, merged: &MergedContext) -> BrainMemoryContext {
        let mut memory = BrainMemoryContext::from(merged);
        memory.stale_sparks = self.stale_sparks_context();
        memory
    }

    /// Attach pending clarification note for the Brain (Memory owns the state).
    pub fn enrich_with_pending(&self, memory: &mut BrainMemoryContext, conversation_id: &str) {
        let Some(pending) = self.get_pending_clarification(conversation_id) else {
            return;
        };
        if pending.tool.is_empty() {
            return;
        }
        let note = format!(
            "Pending clarification for tool `{}`. Partial tool_input JSON: {}. Still need: {}. Merge the user's latest reply into a complete tool_input.",
            pending.tool,
            pending.tool_input,
            pending.missing_labels.join(", ")
        );
        memory.working = Some(match memory.working.take() {
            Some(existing) if !existing.trim().is_empty() => format!("{existing}\n\n{note}"),
            _ => note,
        });
    }

    fn pending_key(conversation_id: &str) -> String {
        format!("pending_clarification:{conversation_id}")
    }

    pub fn get_pending_clarification(
        &self,
        conversation_id: &str,
    ) -> Option<PendingClarification> {
        let key = Self::pending_key(conversation_id);
        if let Some(raw) = self.db.get_runtime_state(&key).ok().flatten() {
            if !raw.trim().is_empty() {
                return serde_json::from_str(&raw).ok();
            }
        }
        // Migrate legacy settings key → Memory runtime state.
        let legacy_key = format!("clarification_pending:{conversation_id}");
        let raw = self.db.get_setting(&legacy_key).ok().flatten()?;
        if raw.trim().is_empty() {
            return None;
        }
        let pending: PendingClarification = serde_json::from_str(&raw).ok()?;
        self.set_pending_clarification(conversation_id, pending.clone());
        let _ = self.db.set_setting(&legacy_key, "");
        Some(pending)
    }

    pub fn set_pending_clarification(
        &self,
        conversation_id: &str,
        mut pending: PendingClarification,
    ) {
        pending.conversation_id = conversation_id.to_string();
        if let Ok(raw) = serde_json::to_string(&pending) {
            let _ = self
                .db
                .set_runtime_state(&Self::pending_key(conversation_id), &raw);
        }
    }

    pub fn clear_pending_clarification(&self, conversation_id: &str) {
        let _ = self
            .db
            .delete_runtime_state(&Self::pending_key(conversation_id));
    }

    fn stale_sparks_context(&self) -> Option<String> {
        let sparks = self
            .db
            .get_stale_sparks(SPARK_STALE_AGE_MS, SPARK_NUDGE_COOLDOWN_MS)
            .ok()?;
        if sparks.is_empty() {
            return None;
        }
        Some(Database::format_stale_sparks_context(&sparks))
    }

    pub async fn finish_task(
        &self,
        state: &AppState,
        conversation_id: &str,
        outcome: &str,
        history: Vec<HistoryMessage>,
        followups: Vec<MemoryEvent>,
    ) {
        let ctx = self.ctx(conversation_id);
        let _ = self.intelligence.on_task_complete(&ctx, outcome).await;
        process_memory_followups(state, &ctx, followups, &history).await;
        self.intelligence.spawn_maintenance(ctx).await;
    }

    /// Indexing / maintenance — Intelligence stays internal.
    pub async fn on_memory_saved(
        &self,
        ctx: &MemoryContext,
        kind: MemoryKind,
        id: &str,
        payload: &serde_json::Value,
    ) {
        if let Err(e) = self
            .intelligence
            .on_memory_saved(ctx, kind, id, payload)
            .await
        {
            warn!(error = %e, kind = ?kind, "memory indexing failed");
        }
    }

    pub fn spawn_index_saved(&self, ctx: &MemoryContext, saved: &[SavedMemory]) {
        for item in saved {
            let api = self.clone();
            let ctx = ctx.clone();
            let kind = item.kind;
            let id = item.id.clone();
            let payload = item.payload.clone();
            tauri::async_runtime::spawn(async move {
                api.on_memory_saved(&ctx, kind, &id, &payload).await;
            });
        }
    }

    pub async fn spawn_reindex(&self) {
        let ctx = MemoryContext {
            workspace_path: self.project_root.clone(),
            conversation_id: None,
            task_id: None,
        };
        self.intelligence.spawn_reindex(ctx).await;
    }

    pub async fn reindex_workspace(&self) -> Result<usize, String> {
        let ctx = MemoryContext {
            workspace_path: self.project_root.clone(),
            conversation_id: None,
            task_id: None,
        };
        self.intelligence
            .reindex_workspace(&ctx)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn run_global_maintenance(&self) -> Result<(), String> {
        let ctx = MemoryContext {
            workspace_path: self.project_root.clone(),
            conversation_id: None,
            task_id: None,
        };
        self.intelligence
            .run_maintenance(&ctx)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn index_saved_sync(&self, ctx: &MemoryContext, saved: &[SavedMemory]) {
        for item in saved {
            self.on_memory_saved(ctx, item.kind, &item.id, &item.payload)
                .await;
        }
    }

    pub async fn on_extraction_saved(&self, ctx: &MemoryContext, data: &serde_json::Value) {
        if let Err(e) = self.intelligence.on_extraction_saved(ctx, data).await {
            warn!(error = %e, "extraction indexing failed");
        }
    }

    pub fn workspace_summary(&self, ctx: &MemoryContext) -> Result<String, String> {
        self.manager
            .workspace_summary(ctx)
            .map_err(|e| e.to_string())
    }
}

fn fallback_context(db: &Database, conversation_id: &str) -> MergedContext {
    let messages = db.get_messages(conversation_id).unwrap_or_default();
    let conversation_messages: Vec<HistoryMessage> = messages
        .iter()
        .map(|m| HistoryMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();
    MergedContext {
        handover: None,
        sections: vec![],
        conversation_messages,
        estimated_tokens: 0,
    }
}

#[cfg(test)]
mod architecture_tests {
    use super::*;
    use buddy_clarification::PendingClarification;
    use std::path::PathBuf;

    fn temp_db() -> Database {
        let dir = std::env::temp_dir().join(format!("buddy-arch-{}", uuid_like()));
        let _ = std::fs::create_dir_all(&dir);
        Database::open(&dir.join("test.db")).expect("open temp db")
    }

    fn uuid_like() -> String {
        format!(
            "{}{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    #[test]
    fn memory_soft_fail_keeps_history() {
        let db = temp_db();
        let conv = db
            .create_conversation("soft-fail")
            .expect("create conversation");
        db.add_message(&conv.id, "user", "hello")
            .expect("add message");
        let ctx = fallback_context(&db, &conv.id);
        assert!(ctx.sections.is_empty());
        assert_eq!(ctx.conversation_messages.len(), 1);
        assert_eq!(ctx.conversation_messages[0].content, "hello");
    }

    #[test]
    fn pending_clarification_lives_in_runtime_state() {
        let db = Arc::new(temp_db());
        let memory_manager = Arc::new(MemoryManager::new(db.clone()));
        let intelligence = Arc::new(IntelligenceService::new(
            db.clone(),
            memory_manager.clone(),
            "http://127.0.0.1:9".into(),
        ));
        let api = MemoryApi::new(
            memory_manager,
            intelligence,
            db,
            PathBuf::from("/tmp"),
        );
        let pending = PendingClarification {
            tool: "calendar.create_event".into(),
            tool_input: r#"{"title":"Meet"}"#.into(),
            missing_labels: vec!["date and time".into()],
            conversation_id: String::new(),
        };
        api.set_pending_clarification("c1", pending);
        let loaded = api.get_pending_clarification("c1").expect("pending");
        assert_eq!(loaded.tool, "calendar.create_event");
        assert_eq!(loaded.conversation_id, "c1");
        api.clear_pending_clarification("c1");
        assert!(api.get_pending_clarification("c1").is_none());
    }
}

/// Archive conversation before delete — Memory policy.
pub async fn archive_conversation(
    state: &AppState,
    conversation_id: &str,
    title: &str,
    history: &[HistoryMessage],
) {
    if history.is_empty() {
        return;
    }
    let ctx = MemoryContext {
        workspace_path: state.project_root.clone(),
        conversation_id: Some(conversation_id.to_string()),
        task_id: None,
    };
    let brain_ok = ProcessManager::check_brain_ready(state).await;
    if brain_ok {
        if let Err(e) =
            archive_conversation_to_memory(state, &ctx, title, conversation_id, history).await
        {
            warn!(error = %e, "conversation archive failed, using fallback");
            let _ =
                save_fallback_conversation_archive(state, &ctx, title, conversation_id, history)
                    .await;
        }
    } else {
        let _ =
            save_fallback_conversation_archive(state, &ctx, title, conversation_id, history).await;
    }
}

pub fn apply_plan_memory_side_effects(
    api: &MemoryApi,
    ctx: &MemoryContext,
    reasoning: &str,
    task_state: Option<&TaskState>,
    preference: Option<(String, String, f64, String)>,
    decision: Option<(String, String)>,
) {
    if let Some(task_state) = task_state {
        match task_state {
            TaskState::Started => {
                let _ = api.store_event(
                    ctx,
                    MemoryEvent::TaskStarted {
                        objective: reasoning.to_string(),
                        plan: Some(reasoning.to_string()),
                        files: vec![],
                    },
                );
            }
            TaskState::Updated => {
                let _ = api.store_event(
                    ctx,
                    MemoryEvent::TaskUpdated {
                        objective: None,
                        plan: Some(reasoning.to_string()),
                        files: None,
                        notes: None,
                    },
                );
            }
            TaskState::Completed => {}
        }
    }
    if let Some((key, value, confidence, source)) = preference {
        let _ = api.store_event(
            ctx,
            MemoryEvent::PreferenceDetected {
                key,
                value,
                confidence,
                source,
            },
        );
    }
    if let Some((decision, reason)) = decision {
        let _ = api.store_event(
            ctx,
            MemoryEvent::DecisionRecorded { decision, reason },
        );
    }
}
