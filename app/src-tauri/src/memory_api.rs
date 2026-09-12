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
use crate::work_item::{
    hydrate_from_legacy, work_item_key, PendingApproval, WorkItem,
};

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
        memory.active_sparks = self.active_sparks_context();
        memory.open_todos = self.db.format_open_todos_context();
        let target: f64 = self
            .db
            .get_setting_or("fitness_calorie_target", "2500")
            .parse()
            .unwrap_or(2500.0);
        memory.fitness = self.db.format_fitness_digest(target);
        memory.study = self.db.format_study_digest();
        memory.money = self.db.format_money_digest();
        memory.socials = self.db.format_socials_digest();
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
            pending.labels().join(", ")
        );
        memory.working = Some(match memory.working.take() {
            Some(existing) if !existing.trim().is_empty() => format!("{existing}\n\n{note}"),
            _ => note,
        });
    }

    fn pending_key(conversation_id: &str) -> String {
        format!("pending_clarification:{conversation_id}")
    }

    fn agent_turn_key(conversation_id: &str) -> String {
        format!("agent_turn:{conversation_id}")
    }

    pub fn get_work_item(&self, conversation_id: &str) -> Option<WorkItem> {
        if let Some(raw) = self
            .db
            .get_runtime_state(&work_item_key(conversation_id))
            .ok()
            .flatten()
        {
            if !raw.trim().is_empty() {
                if let Ok(item) = serde_json::from_str::<WorkItem>(&raw) {
                    return Some(item);
                }
            }
        }
        let pending = self.read_legacy_pending(conversation_id);
        let turn = self
            .db
            .get_runtime_state(&Self::agent_turn_key(conversation_id))
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty());
        let item = hydrate_from_legacy(conversation_id, pending, turn.as_deref())?;
        self.set_work_item(item.clone());
        self.delete_legacy_workflow_keys(conversation_id);
        Some(item)
    }

    pub fn set_work_item(&self, mut item: WorkItem) {
        if item.conversation_id.trim().is_empty() {
            return;
        }
        item.touch();
        if let Ok(raw) = serde_json::to_string(&item) {
            let _ = self
                .db
                .set_runtime_state(&work_item_key(&item.conversation_id), &raw);
        }
    }

    pub fn clear_work_item(&self, conversation_id: &str) {
        let _ = self
            .db
            .delete_runtime_state(&work_item_key(conversation_id));
        self.delete_legacy_workflow_keys(conversation_id);
    }

    fn persist_or_drop(&self, item: WorkItem) {
        if item.is_idle() {
            self.clear_work_item(&item.conversation_id);
        } else {
            self.set_work_item(item);
        }
    }

    fn work_item_or_new(&self, conversation_id: &str) -> WorkItem {
        self.get_work_item(conversation_id)
            .unwrap_or_else(|| WorkItem::new(conversation_id))
    }

    fn read_legacy_pending(&self, conversation_id: &str) -> Option<PendingClarification> {
        let key = Self::pending_key(conversation_id);
        if let Some(raw) = self.db.get_runtime_state(&key).ok().flatten() {
            if !raw.trim().is_empty() {
                if let Ok(pending) = serde_json::from_str::<PendingClarification>(&raw) {
                    return Some(pending);
                }
            }
        }
        let legacy_key = format!("clarification_pending:{conversation_id}");
        let raw = self.db.get_setting(&legacy_key).ok().flatten()?;
        if raw.trim().is_empty() {
            return None;
        }
        serde_json::from_str(&raw).ok()
    }

    fn delete_legacy_workflow_keys(&self, conversation_id: &str) {
        let _ = self
            .db
            .delete_runtime_state(&Self::pending_key(conversation_id));
        let _ = self
            .db
            .delete_runtime_state(&Self::agent_turn_key(conversation_id));
        let legacy_key = format!("clarification_pending:{conversation_id}");
        let _ = self.db.set_setting(&legacy_key, "");
    }

    pub fn get_pending_clarification(
        &self,
        conversation_id: &str,
    ) -> Option<PendingClarification> {
        self.get_work_item(conversation_id)
            .and_then(|item| item.pending_clarification)
    }

    pub fn set_pending_clarification(
        &self,
        conversation_id: &str,
        mut pending: PendingClarification,
    ) {
        pending.conversation_id = conversation_id.to_string();
        let mut item = self.work_item_or_new(conversation_id);
        item.set_pending(pending);
        self.set_work_item(item);
        self.delete_legacy_workflow_keys(conversation_id);
    }

    pub fn clear_pending_clarification(&self, conversation_id: &str) {
        if let Some(mut item) = self.get_work_item(conversation_id) {
            item.clear_pending();
            self.persist_or_drop(item);
        }
        let _ = self
            .db
            .delete_runtime_state(&Self::pending_key(conversation_id));
        let legacy_key = format!("clarification_pending:{conversation_id}");
        let _ = self.db.set_setting(&legacy_key, "");
    }

    pub fn get_agent_turn(&self, conversation_id: &str) -> Option<String> {
        if let Some(item) = self.get_work_item(conversation_id) {
            if let Some(raw) = item.transcript_json() {
                return Some(raw);
            }
        }
        self.db
            .get_runtime_state(&Self::agent_turn_key(conversation_id))
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty())
    }

    pub fn set_agent_turn(&self, conversation_id: &str, raw: &str) {
        let mut item = self.work_item_or_new(conversation_id);
        item.merge_turn_blob(raw);
        self.set_work_item(item);
        let _ = self
            .db
            .delete_runtime_state(&Self::agent_turn_key(conversation_id));
    }

    pub fn clear_agent_turn(&self, conversation_id: &str) {
        if let Some(mut item) = self.get_work_item(conversation_id) {
            item.clear_transcript();
            self.persist_or_drop(item);
        }
        let _ = self
            .db
            .delete_runtime_state(&Self::agent_turn_key(conversation_id));
    }

    pub fn attach_approval(&self, conversation_id: &str, approval: PendingApproval) {
        if conversation_id.trim().is_empty() {
            return;
        }
        let mut item = self.work_item_or_new(conversation_id);
        item.set_approval(approval);
        self.set_work_item(item);
    }

    pub fn clear_approval(&self, conversation_id: &str) {
        if let Some(mut item) = self.get_work_item(conversation_id) {
            item.clear_approval();
            self.persist_or_drop(item);
        }
    }

    fn turn_trace_key(conversation_id: &str) -> String {
        format!("turn_trace:{conversation_id}")
    }

    pub fn get_turn_trace(&self, conversation_id: &str) -> Option<String> {
        self.db
            .get_runtime_state(&Self::turn_trace_key(conversation_id))
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty())
    }

    pub fn set_turn_trace(&self, conversation_id: &str, raw: &str) {
        let _ = self
            .db
            .set_runtime_state(&Self::turn_trace_key(conversation_id), raw);
    }

    fn last_look_key(conversation_id: &str) -> String {
        format!("last_look:{conversation_id}")
    }

    pub fn get_last_look_raw(&self, conversation_id: &str) -> Option<String> {
        self.db
            .get_runtime_state(&Self::last_look_key(conversation_id))
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty())
    }

    pub fn set_last_look_raw(&self, conversation_id: &str, raw: &str) {
        let _ = self
            .db
            .set_runtime_state(&Self::last_look_key(conversation_id), raw);
    }

    pub fn clear_last_look(&self, conversation_id: &str) {
        let _ = self
            .db
            .delete_runtime_state(&Self::last_look_key(conversation_id));
    }

    fn last_life_look_key(conversation_id: &str) -> String {
        format!("last_life_look:{conversation_id}")
    }

    pub fn get_last_life_look_raw(&self, conversation_id: &str) -> Option<String> {
        self.db
            .get_runtime_state(&Self::last_life_look_key(conversation_id))
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty())
    }

    pub fn set_last_life_look_raw(&self, conversation_id: &str, raw: &str) {
        let _ = self
            .db
            .set_runtime_state(&Self::last_life_look_key(conversation_id), raw);
    }

    pub fn clear_last_life_look(&self, conversation_id: &str) {
        let _ = self
            .db
            .delete_runtime_state(&Self::last_life_look_key(conversation_id));
    }

    pub fn set_preference_setting(&self, key: &str, value: &str) {
        let _ = self.db.set_setting(key, value);
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

    fn active_sparks_context(&self) -> Option<String> {
        let sparks = self.db.list_sparks(Some("active")).ok()?;
        if sparks.is_empty() {
            return None;
        }
        let limited: Vec<_> = sparks.into_iter().take(15).collect();
        Some(Database::format_active_sparks_context(&limited))
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
            "{}-{:?}-{}",
            std::process::id(),
            std::thread::current().id(),
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
            missing: vec![],
            missing_labels: vec!["date and time".into()],
            conversation_id: String::new(),
            follow_up: false,
            agent_scratchpad: None,
            agent_goal: None,
            phase: Default::default(),
            last_proposal: None,
        };
        api.set_pending_clarification("c1", pending);
        let loaded = api.get_pending_clarification("c1").expect("pending");
        assert_eq!(loaded.tool, "calendar.create_event");
        assert_eq!(loaded.conversation_id, "c1");
        api.clear_pending_clarification("c1");
        assert!(api.get_pending_clarification("c1").is_none());
    }

    #[test]
    fn work_item_is_authoritative_and_migrates_legacy_keys() {
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
            db.clone(),
            PathBuf::from("/tmp"),
        );
        let pending = PendingClarification {
            tool: "todo.add".into(),
            tool_input: r#"{"title":"milk"}"#.into(),
            missing: vec![],
            missing_labels: vec!["deadline".into()],
            conversation_id: String::new(),
            follow_up: false,
            agent_scratchpad: None,
            agent_goal: Some("buy milk".into()),
            phase: Default::default(),
            last_proposal: None,
        };
        let _ = db.set_runtime_state(
            "pending_clarification:c2",
            &serde_json::to_string(&pending).unwrap(),
        );
        let _ = db.set_runtime_state(
            "agent_turn:c2",
            r#"{"goal":"buy milk","messages":[{"role":"user","content":"remind me"}],"scratchpad":[]}"#,
        );

        let item = api.get_work_item("c2").expect("hydrated");
        assert_eq!(item.objective, "buy milk");
        assert!(item.pending_clarification.is_some());
        assert!(item.transcript.is_some());
        assert!(db
            .get_runtime_state("work_item:c2")
            .ok()
            .flatten()
            .is_some());
        assert!(db
            .get_runtime_state("pending_clarification:c2")
            .ok()
            .flatten()
            .is_none());
        assert!(db
            .get_runtime_state("agent_turn:c2")
            .ok()
            .flatten()
            .is_none());
    }

    #[test]
    fn new_writes_do_not_use_agent_turn_key() {
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
            db.clone(),
            PathBuf::from("/tmp"),
        );
        api.set_agent_turn(
            "c3",
            r#"{"goal":"plan Friday","messages":[{"role":"user","content":"plan Friday"}],"scratchpad":[]}"#,
        );
        api.set_agent_turn(
            "c3",
            r#"{"goal":"plan Friday","scratchpad":[{"tool":"calendar.look","summary":"looked"}]}"#,
        );
        assert!(db
            .get_runtime_state("agent_turn:c3")
            .ok()
            .flatten()
            .is_none());
        let raw = api.get_agent_turn("c3").expect("transcript");
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["messages"].as_array().unwrap().len(), 1);
        assert_eq!(v["scratchpad"][0]["tool"], "calendar.look");
        let item = api.get_work_item("c3").unwrap();
        assert_eq!(item.observations.len(), 1);
        assert_eq!(item.observations[0].payload.as_deref(), Some("calendar.look"));
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
                key: key.clone(),
                value: value.clone(),
                confidence,
                source,
            },
        );
        // High-confidence prefs also land in settings so Clarification memory_keys work.
        if confidence >= 0.9 && !key.trim().is_empty() {
            api.set_preference_setting(&key, &value);
        }
    }
    if let Some((decision, reason)) = decision {
        let _ = api.store_event(
            ctx,
            MemoryEvent::DecisionRecorded { decision, reason },
        );
    }
}
