use std::sync::Arc;

use crate::error::MemoryError;
use crate::storage::{MemoryRow, SqliteStorageBackend, StorageBackend, StorageQuery};
use crate::memory_trait::Memory;
use crate::types::{chrono_now, MemoryContext, MemoryKind, MemoryRecord, RetrieveQuery};

pub struct HandoverMemory {
    storage: Arc<SqliteStorageBackend>,
}

impl HandoverMemory {
    pub fn new(storage: Arc<SqliteStorageBackend>) -> Self {
        Self { storage }
    }
}

impl Memory for HandoverMemory {
    fn kind(&self) -> MemoryKind {
        MemoryKind::Handover
    }

    fn save(&self, ctx: &MemoryContext, record: MemoryRecord) -> Result<String, MemoryError> {
        let now = chrono_now();
        let row = MemoryRow {
            id: record.id.unwrap_or_default(),
            workspace_path: ctx.workspace_path.display().to_string(),
            created_at: now,
            updated_at: now,
            payload: record.payload.to_string(),
        };
        self.storage
            .insert(MemoryKind::Handover, &row)
            .map_err(Into::into)
    }

    fn retrieve(&self, query: &RetrieveQuery) -> Result<Vec<MemoryRecord>, MemoryError> {
        let rows = self.storage.query(StorageQuery {
            table: MemoryKind::Handover,
            workspace_path: query.workspace_path.display().to_string(),
            limit: query.limit.or(Some(1)),
            order_desc: true,
        })?;
        Ok(rows
            .into_iter()
            .map(|r| MemoryRecord {
                id: Some(r.id),
                kind: MemoryKind::Handover,
                payload: serde_json::from_str(&r.payload).unwrap_or_default(),
                created_at: Some(r.created_at),
                updated_at: Some(r.updated_at),
            })
            .collect())
    }

    fn update(&self, id: &str, record: MemoryRecord) -> Result<(), MemoryError> {
        let row = MemoryRow {
            id: id.to_string(),
            workspace_path: String::new(),
            created_at: record.created_at.unwrap_or_else(chrono_now),
            updated_at: chrono_now(),
            payload: record.payload.to_string(),
        };
        self.storage.update(MemoryKind::Handover, id, &row)?;
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), MemoryError> {
        self.storage.delete(MemoryKind::Handover, id)?;
        Ok(())
    }

    fn summarize(&self, query: &RetrieveQuery) -> Result<String, MemoryError> {
        let records = self.retrieve(query)?;
        let Some(record) = records.first() else {
            return Ok(String::new());
        };
        if let Some(summary) = record.payload.get("summary") {
            if let Some(text) = summary.as_str() {
                return Ok(text.to_string());
            }
            if let Some(obj) = summary.as_object() {
                let mut parts = Vec::new();
                for (key, value) in obj {
                    if let Some(s) = value.as_str() {
                        parts.push(format!("{key}: {s}"));
                    } else if let Some(arr) = value.as_array() {
                        let items: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                        if !items.is_empty() {
                            parts.push(format!("{key}: {}", items.join(", ")));
                        }
                    }
                }
                return Ok(parts.join("\n"));
            }
            return Ok(summary.to_string());
        }
        Ok(record.payload.to_string())
    }
}
