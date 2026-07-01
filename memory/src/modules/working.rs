use std::sync::Arc;

use uuid::Uuid;

use crate::error::MemoryError;
use crate::storage::{MemoryRow, SqliteStorageBackend, StorageBackend, StorageQuery};
use crate::memory_trait::Memory;
use crate::types::{chrono_now, MemoryContext, MemoryKind, MemoryRecord, RetrieveQuery};

pub struct WorkingMemory {
    storage: Arc<SqliteStorageBackend>,
}

impl WorkingMemory {
    pub fn new(storage: Arc<SqliteStorageBackend>) -> Self {
        Self { storage }
    }
}

impl Memory for WorkingMemory {
    fn kind(&self) -> MemoryKind {
        MemoryKind::Working
    }

    fn save(&self, ctx: &MemoryContext, record: MemoryRecord) -> Result<String, MemoryError> {
        let now = chrono_now();
        let task_id = record
            .payload
            .get("task_id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| ctx.task_id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let mut payload = record.payload.clone();
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("task_id".into(), serde_json::json!(task_id));
            obj.insert("status".into(), serde_json::json!("active"));
        }

        let row = MemoryRow {
            id: record.id.unwrap_or_default(),
            workspace_path: ctx.workspace_path.display().to_string(),
            created_at: now,
            updated_at: now,
            payload: payload.to_string(),
            ..Default::default()
        };
        let id = self.storage.insert(MemoryKind::Working, &row)?;
        Ok(id)
    }

    fn retrieve(&self, query: &RetrieveQuery) -> Result<Vec<MemoryRecord>, MemoryError> {
        let rows = self.storage.query(StorageQuery {
            table: MemoryKind::Working,
            workspace_path: query.workspace_path.display().to_string(),
            limit: query.limit.or(Some(1)),
            order_desc: true,
        })?;
        Ok(rows
            .into_iter()
            .filter(|r| {
                serde_json::from_str::<serde_json::Value>(&r.payload)
                    .ok()
                    .and_then(|p| p.get("status").and_then(|s| s.as_str()).map(|s| s == "active"))
                    .unwrap_or(false)
            })
            .map(|r| MemoryRecord {
                id: Some(r.id),
                kind: MemoryKind::Working,
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
        self.storage.update(MemoryKind::Working, id, &row)?;
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), MemoryError> {
        self.storage.delete(MemoryKind::Working, id)?;
        Ok(())
    }

    fn summarize(&self, query: &RetrieveQuery) -> Result<String, MemoryError> {
        let records = self.retrieve(query)?;
        let Some(record) = records.first() else {
            return Ok(String::new());
        };
        let p = &record.payload;
        let objective = p.get("objective").and_then(|v| v.as_str()).unwrap_or("");
        let plan = p.get("plan").and_then(|v| v.as_str()).unwrap_or("");
        let files = p
            .get("files")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let notes = p.get("notes").and_then(|v| v.as_str()).unwrap_or("");
        Ok(format!(
            "Active task: {objective}\nPlan: {plan}\nFiles: {files}\nNotes: {notes}"
        ))
    }
}
