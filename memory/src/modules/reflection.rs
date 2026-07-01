use std::sync::Arc;

use crate::error::MemoryError;
use crate::storage::{MemoryRow, SqliteStorageBackend, StorageBackend, StorageQuery};
use crate::memory_trait::Memory;
use crate::types::{chrono_now, MemoryContext, MemoryKind, MemoryRecord, RetrieveQuery};

pub struct ReflectionMemory {
    storage: Arc<SqliteStorageBackend>,
}

impl ReflectionMemory {
    pub fn new(storage: Arc<SqliteStorageBackend>) -> Self {
        Self { storage }
    }
}

impl Memory for ReflectionMemory {
    fn kind(&self) -> MemoryKind {
        MemoryKind::Reflection
    }

    fn save(&self, ctx: &MemoryContext, record: MemoryRecord) -> Result<String, MemoryError> {
        let now = chrono_now();
        let row = MemoryRow {
            id: record.id.unwrap_or_default(),
            workspace_path: ctx.workspace_path.display().to_string(),
            created_at: now,
            updated_at: now,
            payload: record.payload.to_string(),
            ..Default::default()
        };
        self.storage
            .insert(MemoryKind::Reflection, &row)
            .map_err(Into::into)
    }

    fn retrieve(&self, query: &RetrieveQuery) -> Result<Vec<MemoryRecord>, MemoryError> {
        let rows = self.storage.query(StorageQuery {
            table: MemoryKind::Reflection,
            workspace_path: query.workspace_path.display().to_string(),
            limit: query.limit.or(Some(20)),
            order_desc: true,
        })?;
        Ok(rows
            .into_iter()
            .map(|r| MemoryRecord {
                id: Some(r.id),
                kind: MemoryKind::Reflection,
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
        self.storage.update(MemoryKind::Reflection, id, &row)?;
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), MemoryError> {
        self.storage.delete(MemoryKind::Reflection, id)?;
        Ok(())
    }

    fn summarize(&self, query: &RetrieveQuery) -> Result<String, MemoryError> {
        let records = self.retrieve(query)?;
        if records.is_empty() {
            return Ok(String::new());
        }
        let lines: Vec<String> = records
            .iter()
            .take(5)
            .filter_map(|r| {
                let attempted = r.payload.get("attempted")?.as_str()?;
                let successful = r.payload.get("successful")?.as_bool()?;
                let lessons = r.payload.get("lessons")?.as_str()?;
                Some(format!(
                    "- {attempted} (success={successful}): {lessons}"
                ))
            })
            .collect();
        Ok(format!("Recent reflections:\n{}", lines.join("\n")))
    }
}
