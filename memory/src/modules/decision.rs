use std::sync::Arc;

use crate::error::MemoryError;
use crate::storage::{MemoryRow, SqliteStorageBackend, StorageBackend, StorageQuery};
use crate::memory_trait::Memory;
use crate::types::{chrono_now, MemoryContext, MemoryKind, MemoryRecord, RetrieveQuery};

pub struct DecisionMemory {
    storage: Arc<SqliteStorageBackend>,
}

impl DecisionMemory {
    pub fn new(storage: Arc<SqliteStorageBackend>) -> Self {
        Self { storage }
    }
}

impl Memory for DecisionMemory {
    fn kind(&self) -> MemoryKind {
        MemoryKind::Decision
    }

    fn save(&self, ctx: &MemoryContext, record: MemoryRecord) -> Result<String, MemoryError> {
        let now = chrono_now();
        let mut payload = record.payload.clone();
        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "date".into(),
                serde_json::json!(chrono_now()),
            );
        }
        let row = MemoryRow {
            id: record.id.unwrap_or_default(),
            workspace_path: ctx.workspace_path.display().to_string(),
            created_at: now,
            updated_at: now,
            payload: payload.to_string(),
        };
        self.storage
            .insert(MemoryKind::Decision, &row)
            .map_err(Into::into)
    }

    fn retrieve(&self, query: &RetrieveQuery) -> Result<Vec<MemoryRecord>, MemoryError> {
        let rows = self.storage.query(StorageQuery {
            table: MemoryKind::Decision,
            workspace_path: query.workspace_path.display().to_string(),
            limit: query.limit.or(Some(50)),
            order_desc: true,
        })?;
        let mut records: Vec<MemoryRecord> = rows
            .into_iter()
            .map(|r| MemoryRecord {
                id: Some(r.id),
                kind: MemoryKind::Decision,
                payload: serde_json::from_str(&r.payload).unwrap_or_default(),
                created_at: Some(r.created_at),
                updated_at: Some(r.updated_at),
            })
            .collect();

        if let Some(keywords) = &query.keywords {
            let lower = keywords.to_lowercase();
            records.retain(|r| {
                let decision = r
                    .payload
                    .get("decision")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let reason = r
                    .payload
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                decision.to_lowercase().contains(&lower)
                    || reason.to_lowercase().contains(&lower)
            });
        }

        Ok(records)
    }

    fn update(&self, id: &str, record: MemoryRecord) -> Result<(), MemoryError> {
        let row = MemoryRow {
            id: id.to_string(),
            workspace_path: String::new(),
            created_at: record.created_at.unwrap_or_else(chrono_now),
            updated_at: chrono_now(),
            payload: record.payload.to_string(),
        };
        self.storage.update(MemoryKind::Decision, id, &row)?;
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), MemoryError> {
        self.storage.delete(MemoryKind::Decision, id)?;
        Ok(())
    }

    fn summarize(&self, query: &RetrieveQuery) -> Result<String, MemoryError> {
        let records = self.retrieve(query)?;
        if records.is_empty() {
            return Ok(String::new());
        }
        let lines: Vec<String> = records
            .iter()
            .take(15)
            .filter_map(|r| {
                let decision = r.payload.get("decision")?.as_str()?;
                let reason = r.payload.get("reason")?.as_str()?;
                Some(format!("- {decision}: {reason}"))
            })
            .collect();
        Ok(format!("Architectural decisions:\n{}", lines.join("\n")))
    }
}
