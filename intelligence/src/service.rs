use std::sync::Arc;

use buddy_database::Database;
use buddy_memory::{
    MemoryContext, MemoryKind, MemoryManager, MergedContext, DEFAULT_TOKEN_BUDGET,
};
use buddy_memory::default_importance;
use tracing::{info, warn};

use crate::context_builder::ContextBuilder;
use crate::error::IntelligenceError;
use crate::knowledge_graph::KnowledgeGraph;
use crate::learning::LearningEngine;
use crate::maintenance::{MaintenanceEngine, MaintenanceReport};
use crate::semantic::{IndexRecord, SemanticSearch, SqliteEmbeddingBackend};
use crate::search_text::extract_search_text;
use crate::workspace::WorkspaceIntel;

pub struct IntelligenceService {
    memory: Arc<MemoryManager>,
    search: Arc<SqliteEmbeddingBackend>,
    kg: KnowledgeGraph,
    learning: LearningEngine,
    workspace_intel: WorkspaceIntel,
    maintenance: MaintenanceEngine,
    token_budget: usize,
    brain_url: String,
}

impl IntelligenceService {
    pub fn new(db: Arc<Database>, memory: Arc<MemoryManager>, brain_url: String) -> Self {
        let search = Arc::new(SqliteEmbeddingBackend::new(db.clone(), brain_url.clone()));
        let maintenance = MaintenanceEngine::new(db.clone(), search.clone(), memory.clone());
        Self {
            memory,
            search,
            kg: KnowledgeGraph::new(db.clone()),
            learning: LearningEngine::new(db.clone()),
            workspace_intel: WorkspaceIntel::new(db),
            maintenance,
            token_budget: DEFAULT_TOKEN_BUDGET,
            brain_url,
        }
    }

    pub fn brain_url(&self) -> &str {
        &self.brain_url
    }

    pub fn memory(&self) -> &Arc<MemoryManager> {
        &self.memory
    }

    pub async fn build_context(
        &self,
        ctx: &MemoryContext,
        query: &str,
    ) -> Result<MergedContext, IntelligenceError> {
        ContextBuilder::build(
            &self.memory,
            self.search.as_ref(),
            &self.kg,
            &self.learning,
            &self.workspace_intel,
            ctx,
            query,
            self.token_budget,
        )
        .await
    }

    pub async fn on_memory_saved(
        &self,
        ctx: &MemoryContext,
        kind: MemoryKind,
        id: &str,
        payload: &serde_json::Value,
    ) -> Result<(), IntelligenceError> {
        if kind == MemoryKind::Conversation {
            return Ok(());
        }

        let search_text = extract_search_text(kind, payload);
        if search_text.is_empty() {
            return Ok(());
        }

        let workspace = ctx.workspace_path.display().to_string();
        let importance = payload
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(default_importance(kind));
        let updated_at = buddy_database::chrono_now();

        let embedding = self.search.embed_with_fallback(&search_text).await;

        self.search
            .index(IndexRecord {
                id: id.to_string(),
                kind,
                workspace_path: workspace.clone(),
                search_text,
                embedding,
                importance,
                updated_at,
            })
            .await?;

        // Structured knowledge graph ingestion
        match kind {
            MemoryKind::Decision => {
                let decision = payload.get("decision").and_then(|v| v.as_str()).unwrap_or("");
                let reason = payload.get("reason").and_then(|v| v.as_str()).unwrap_or("");
                let _ = self.kg.ingest_decision(&workspace, decision, reason, Some(id));
                let _ = self.learning.observe_decision(&workspace, decision);
            }
            MemoryKind::Preference => {
                let key = payload.get("key").and_then(|v| v.as_str()).unwrap_or("");
                let value = payload.get("value").and_then(|v| v.as_str()).unwrap_or("");
                let _ = self.kg.ingest_preference(&workspace, key, value);
                let _ = self.learning.observe_preference(&workspace, key, value);
            }
            MemoryKind::Project => {
                let section = payload.get("section").and_then(|v| v.as_str()).unwrap_or("");
                let content = payload.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let _ = self.kg.ingest_project_section(&workspace, section, content);
            }
            MemoryKind::Tool => {
                let tool = payload.get("tool").and_then(|v| v.as_str()).unwrap_or("");
                let _ = self.learning.observe_tool(&workspace, tool);
            }
            _ => {}
        }

        Ok(())
    }

    pub async fn on_task_complete(
        &self,
        ctx: &MemoryContext,
        outcome: &str,
    ) -> Result<(), IntelligenceError> {
        let workspace = ctx.workspace_path.display().to_string();
        self.learning
            .on_task_complete(&workspace, outcome)
            .map_err(|e| IntelligenceError::Other(e))?;
        let _ = self
            .workspace_intel
            .refresh(&workspace, &self.memory, &self.kg, &self.learning);
        Ok(())
    }

    pub async fn on_extraction_saved(
        &self,
        ctx: &MemoryContext,
        data: &serde_json::Value,
    ) -> Result<(), IntelligenceError> {
        let workspace = ctx.workspace_path.display().to_string();
        if let (Some(entities), Some(relations)) = (
            data.get("entities").and_then(|v| v.as_array()),
            data.get("relations").and_then(|v| v.as_array()),
        ) {
            let _ = self.kg.ingest_llm_entities(&workspace, entities, relations);
        }
        Ok(())
    }

    pub async fn reindex_workspace(&self, ctx: &MemoryContext) -> Result<usize, IntelligenceError> {
        let workspace = ctx.workspace_path.display().to_string();
        let unindexed = self
            .search
            .list_unindexed(&workspace, 500)
            .map_err(|e| IntelligenceError::Search(e))?;

        let mut count = 0;
        for (kind, id, payload_str, importance, updated_at) in unindexed {
            let payload: serde_json::Value =
                serde_json::from_str(&payload_str).unwrap_or_default();
            let search_text = extract_search_text(kind, &payload);
            if search_text.is_empty() {
                continue;
            }
            let embedding = self.search.embed_with_fallback(&search_text).await;
            if self
                .search
                .index(IndexRecord {
                    id: id.clone(),
                    kind,
                    workspace_path: workspace.clone(),
                    search_text,
                    embedding,
                    importance,
                    updated_at,
                })
                .await
                .is_ok()
            {
                count += 1;
            }
        }
        info!(count, workspace = %workspace, "reindex completed");
        Ok(count)
    }

    pub async fn run_maintenance(
        &self,
        ctx: &MemoryContext,
    ) -> Result<MaintenanceReport, IntelligenceError> {
        let workspace = ctx.workspace_path.display().to_string();
        self.maintenance
            .run(
                &workspace,
                &self.kg,
                &self.learning,
                &self.workspace_intel,
            )
            .await
            .map_err(|e| IntelligenceError::Other(e))
    }

    pub async fn spawn_reindex(&self, ctx: MemoryContext) {
        let this = self.clone_for_async();
        tokio::spawn(async move {
            if let Err(e) = this.reindex_workspace(&ctx).await {
                warn!(error = %e, "background reindex failed");
            }
        });
    }

    pub async fn spawn_maintenance(&self, ctx: MemoryContext) {
        let this = self.clone_for_async();
        tokio::spawn(async move {
            if let Err(e) = this.run_maintenance(&ctx).await {
                warn!(error = %e, "background maintenance failed");
            }
        });
    }

    fn clone_for_async(&self) -> IntelligenceServiceRef {
        IntelligenceServiceRef {
            search: self.search.clone(),
            kg: KnowledgeGraph::new(self.search.db()),
            learning: LearningEngine::new(self.search.db()),
            workspace_intel: WorkspaceIntel::new(self.search.db()),
            maintenance: MaintenanceEngine::new(
                self.search.db(),
                self.search.clone(),
                self.memory.clone(),
            ),
        }
    }
}

struct IntelligenceServiceRef {
    search: Arc<SqliteEmbeddingBackend>,
    kg: KnowledgeGraph,
    learning: LearningEngine,
    workspace_intel: WorkspaceIntel,
    maintenance: MaintenanceEngine,
}

impl IntelligenceServiceRef {
    async fn reindex_workspace(&self, ctx: &MemoryContext) -> Result<usize, IntelligenceError> {
        let workspace = ctx.workspace_path.display().to_string();
        let unindexed = self
            .search
            .list_unindexed(&workspace, 500)
            .map_err(|e| IntelligenceError::Search(e))?;
        let mut count = 0;
        for (kind, id, payload_str, importance, updated_at) in unindexed {
            let payload: serde_json::Value =
                serde_json::from_str(&payload_str).unwrap_or_default();
            let search_text = extract_search_text(kind, &payload);
            if search_text.is_empty() {
                continue;
            }
            let embedding = self.search.embed_with_fallback(&search_text).await;
            if self
                .search
                .index(IndexRecord {
                    id,
                    kind,
                    workspace_path: workspace.clone(),
                    search_text,
                    embedding,
                    importance,
                    updated_at,
                })
                .await
                .is_ok()
            {
                count += 1;
            }
        }
        Ok(count)
    }

    async fn run_maintenance(&self, ctx: &MemoryContext) -> Result<MaintenanceReport, IntelligenceError> {
        let workspace = ctx.workspace_path.display().to_string();
        self.maintenance
            .run(&workspace, &self.kg, &self.learning, &self.workspace_intel)
            .await
            .map_err(|e| IntelligenceError::Other(e))
    }
}
