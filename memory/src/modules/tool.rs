use std::sync::Arc;

use crate::error::MemoryError;
use crate::storage::{MemoryRow, SqliteStorageBackend, StorageBackend, StorageQuery};
use crate::memory_trait::Memory;
use crate::types::{chrono_now, MemoryContext, MemoryKind, MemoryRecord, RetrieveQuery, DEFAULT_TOOL_WINDOW};

pub struct ToolMemory {
    storage: Arc<SqliteStorageBackend>,
    window: usize,
}

impl ToolMemory {
    pub fn new(storage: Arc<SqliteStorageBackend>) -> Self {
        Self {
            storage,
            window: DEFAULT_TOOL_WINDOW,
        }
    }
}

impl Memory for ToolMemory {
    fn kind(&self) -> MemoryKind {
        MemoryKind::Tool
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
        self.storage.insert(MemoryKind::Tool, &row).map_err(Into::into)
    }

    fn retrieve(&self, query: &RetrieveQuery) -> Result<Vec<MemoryRecord>, MemoryError> {
        let rows = self.storage.query(StorageQuery {
            table: MemoryKind::Tool,
            workspace_path: query.workspace_path.display().to_string(),
            limit: query.limit.or(Some(self.window)),
            order_desc: true,
        })?;
        Ok(rows
            .into_iter()
            .map(|r| MemoryRecord {
                id: Some(r.id),
                kind: MemoryKind::Tool,
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
        self.storage.update(MemoryKind::Tool, id, &row)?;
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), MemoryError> {
        self.storage.delete(MemoryKind::Tool, id)?;
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
                let tool = r.payload.get("tool")?.as_str()?;
                let success = r.payload.get("success")?.as_bool()?;
                let duration = r.payload.get("duration_ms")?.as_u64()?;
                Some(format!(
                    "- {tool} ({duration}ms, success={success})"
                ))
            })
            .collect();
        Ok(format!("Recent tool executions:\n{}", lines.join("\n")))
    }
}
