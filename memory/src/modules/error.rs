use std::sync::Arc;

use crate::error::MemoryError;
use crate::storage::{MemoryRow, SqliteStorageBackend, StorageBackend, StorageQuery};
use crate::memory_trait::Memory;
use crate::types::{chrono_now, MemoryContext, MemoryKind, MemoryRecord, RetrieveQuery};

pub struct ErrorMemory {
    storage: Arc<SqliteStorageBackend>,
}

impl ErrorMemory {
    pub fn new(storage: Arc<SqliteStorageBackend>) -> Self {
        Self { storage }
    }
}

impl Memory for ErrorMemory {
    fn kind(&self) -> MemoryKind {
        MemoryKind::Error
    }

    fn save(&self, ctx: &MemoryContext, record: MemoryRecord) -> Result<String, MemoryError> {
        let error_text = record
            .payload
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let existing = self.retrieve(&RetrieveQuery {
            workspace_path: ctx.workspace_path.clone(),
            conversation_id: None,
            task_id: None,
            keywords: None,
            limit: Some(100),
        })?;

        if let Some(existing_record) = existing.iter().find(|r| {
            r.payload
                .get("error")
                .and_then(|v| v.as_str())
                == Some(error_text)
        }) {
            let id = existing_record.id.as_deref().unwrap_or("");
            let mut payload = existing_record.payload.clone();
            let freq = payload
                .get("frequency")
                .and_then(|v| v.as_u64())
                .unwrap_or(1)
                + 1;
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("frequency".into(), serde_json::json!(freq));
            }
            self.update(id, MemoryRecord {
                id: Some(id.to_string()),
                kind: MemoryKind::Error,
                payload,
                created_at: existing_record.created_at,
                updated_at: Some(chrono_now()),
            })?;
            return Ok(id.to_string());
        }

        let now = chrono_now();
        let mut payload = record.payload.clone();
        if payload.get("frequency").is_none() {
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("frequency".into(), serde_json::json!(1));
            }
        }
        let row = MemoryRow {
            id: record.id.unwrap_or_default(),
            workspace_path: ctx.workspace_path.display().to_string(),
            created_at: now,
            updated_at: now,
            payload: payload.to_string(),
            ..Default::default()
        };
        self.storage.insert(MemoryKind::Error, &row).map_err(Into::into)
    }

    fn retrieve(&self, query: &RetrieveQuery) -> Result<Vec<MemoryRecord>, MemoryError> {
        let rows = self.storage.query(StorageQuery {
            table: MemoryKind::Error,
            workspace_path: query.workspace_path.display().to_string(),
            limit: query.limit.or(Some(50)),
            order_desc: true,
        })?;
        Ok(rows
            .into_iter()
            .map(|r| MemoryRecord {
                id: Some(r.id),
                kind: MemoryKind::Error,
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
            ..Default::default()
        };
        self.storage.update(MemoryKind::Error, id, &row)?;
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), MemoryError> {
        self.storage.delete(MemoryKind::Error, id)?;
        Ok(())
    }

    fn summarize(&self, query: &RetrieveQuery) -> Result<String, MemoryError> {
        let records = self.retrieve(query)?;
        if records.is_empty() {
            return Ok(String::new());
        }
        let lines: Vec<String> = records
            .iter()
            .take(10)
            .filter_map(|r| {
                let error = r.payload.get("error")?.as_str()?;
                let cause = r.payload.get("cause")?.as_str()?;
                let resolution = r.payload.get("resolution").and_then(|v| v.as_str()).unwrap_or("");
                let freq = r.payload.get("frequency").and_then(|v| v.as_u64()).unwrap_or(1);
                Some(format!(
                    "- {error} (x{freq}): {cause}. Resolution: {resolution}"
                ))
            })
            .collect();
        Ok(format!("Known errors:\n{}", lines.join("\n")))
    }
}
