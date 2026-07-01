use std::sync::Arc;

use buddy_database::Database;
use buddy_memory::MemoryManager;
use rusqlite::params;
use tracing::info;

use crate::knowledge_graph::KnowledgeGraph;
use crate::learning::LearningEngine;
use crate::semantic::SqliteEmbeddingBackend;
use crate::workspace::WorkspaceIntel;

#[derive(Debug, Default)]
pub struct MaintenanceReport {
    pub merged_duplicates: usize,
    pub archived: usize,
    pub removed: usize,
    pub conflicts_detected: usize,
    pub confidence_updated: usize,
}

pub struct MaintenanceEngine {
    db: Arc<Database>,
    _search: Arc<SqliteEmbeddingBackend>,
    _memory: Arc<MemoryManager>,
}

impl MaintenanceEngine {
    pub fn new(
        db: Arc<Database>,
        search: Arc<SqliteEmbeddingBackend>,
        memory: Arc<MemoryManager>,
    ) -> Self {
        Self { db, _search: search, _memory: memory }
    }

    pub async fn run(
        &self,
        workspace_path: &str,
        kg: &KnowledgeGraph,
        learning: &LearningEngine,
        workspace_intel: &WorkspaceIntel,
    ) -> Result<MaintenanceReport, String> {
        let mut report = MaintenanceReport::default();
        report.merged_duplicates = self.merge_duplicates(workspace_path).await?;
        report.archived = self.archive_inactive(workspace_path)?;
        report.conflicts_detected = self.detect_conflicts(workspace_path)?;
        report.confidence_updated = self.decay_stale_confidence(workspace_path)?;

        let _ = workspace_intel.refresh(workspace_path, &self._memory, kg, learning);
        info!(?report, "maintenance completed");
        Ok(report)
    }

    async fn merge_duplicates(&self, workspace_path: &str) -> Result<usize, String> {
        use buddy_memory::MemoryKind;

        let kinds = [
            MemoryKind::Decision,
            MemoryKind::Preference,
            MemoryKind::Reflection,
        ];
        let mut merged = 0;

        for kind in kinds {
            let table = kind.table_name();
            let rows: Vec<(String, Vec<u8>, String)> = self
                .db
                .with_conn(|conn| {
                    let mut stmt = conn.prepare(&format!(
                        "SELECT id, embedding, payload FROM {table} \
                         WHERE workspace_path = ?1 AND embedding IS NOT NULL"
                    ))?;
                    let rows = stmt.query_map(params![workspace_path], |row| {
                        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                    })?;
                    rows.collect::<Result<Vec<_>, _>>()
                        .map_err(buddy_database::DbError::from)
                })
                .map_err(|e| e.to_string())?;

            let mut to_delete = Vec::new();
            for i in 0..rows.len() {
                for j in (i + 1)..rows.len() {
                    let emb_i = crate::semantic::bytes_to_embedding(&rows[i].1);
                    let emb_j = crate::semantic::bytes_to_embedding(&rows[j].1);
                    if emb_i.len() == emb_j.len() {
                        let sim = crate::semantic::cosine_similarity(&emb_i, &emb_j);
                        if sim > 0.92 {
                            to_delete.push(rows[j].0.clone());
                            merged += 1;
                        }
                    }
                }
            }

            for id in to_delete {
                self.db
                    .with_conn(|conn| {
                        conn.execute(
                            &format!("DELETE FROM {table} WHERE id = ?1"),
                            params![id],
                        )?;
                        Ok(())
                    })
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(merged)
    }

    fn archive_inactive(&self, workspace_path: &str) -> Result<usize, String> {
        use buddy_memory::MemoryKind;

        let cutoff = buddy_database::chrono_now() - 90 * 86400;
        let mut archived = 0;

        for kind in [
            MemoryKind::Tool,
            MemoryKind::Reflection,
            MemoryKind::Error,
        ] {
            let table = kind.table_name();
            let count = self
                .db
                .with_conn(|conn| {
                    let updated = conn.execute(
                        &format!(
                            "UPDATE {table} SET payload = json_set(payload, '$.archived', true) \
                             WHERE workspace_path = ?1 AND updated_at < ?2 \
                             AND COALESCE(importance, 0.5) < 0.4 \
                             AND json_extract(payload, '$.archived') IS NULL"
                        ),
                        params![workspace_path, cutoff],
                    )?;
                    Ok(updated)
                })
                .map_err(|e| e.to_string())?;
            archived += count;
        }
        Ok(archived)
    }

    fn detect_conflicts(&self, workspace_path: &str) -> Result<usize, String> {
        let rows: Vec<(String, String)> = self
            .db
            .with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT id, payload FROM memory_preference WHERE workspace_path = ?1",
                )?;
                let rows = stmt.query_map(params![workspace_path], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(buddy_database::DbError::from)
            })
            .map_err(|e| e.to_string())?;

        let mut by_key: std::collections::HashMap<String, Vec<(String, String)>> =
            std::collections::HashMap::new();

        for (id, payload) in rows {
            if let Ok(p) = serde_json::from_str::<serde_json::Value>(&payload) {
                let key = p.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let value = p.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
                by_key.entry(key).or_default().push((id, value));
            }
        }

        let mut conflicts = 0;
        for (_key, values) in by_key {
            if values.len() < 2 {
                continue;
            }
            for i in 0..values.len() {
                for j in (i + 1)..values.len() {
                    if values[i].1.to_lowercase() != values[j].1.to_lowercase() {
                        for (id, _) in [&values[i], &values[j]] {
                            self.db
                                .with_conn(|conn| {
                                    conn.execute(
                                        "UPDATE memory_preference SET importance = COALESCE(importance, 0.7) * 0.5 WHERE id = ?1",
                                        params![id],
                                    )?;
                                    Ok(())
                                })
                                .map_err(|e| e.to_string())?;
                        }
                        conflicts += 1;
                    }
                }
            }
        }
        Ok(conflicts)
    }

    fn decay_stale_confidence(&self, workspace_path: &str) -> Result<usize, String> {
        let cutoff = buddy_database::chrono_now() - 30 * 86400;
        let count = self
            .db
            .with_conn(|conn| {
                let updated = conn.execute(
                    "UPDATE memory_preference SET importance = COALESCE(importance, 0.7) * 0.9 \
                     WHERE workspace_path = ?1 AND updated_at < ?2 \
                     AND json_extract(payload, '$.source') = 'inferred'",
                    params![workspace_path, cutoff],
                )?;
                Ok(updated)
            })
            .map_err(|e| e.to_string())?;
        Ok(count)
    }
}
