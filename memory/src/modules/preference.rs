use std::sync::Arc;

use crate::error::MemoryError;
use crate::storage::{MemoryRow, SqliteStorageBackend, StorageBackend, StorageQuery};
use crate::memory_trait::Memory;
use crate::types::{chrono_now, MemoryContext, MemoryKind, MemoryRecord, RetrieveQuery};

pub struct PreferenceMemory {
    storage: Arc<SqliteStorageBackend>,
}

impl PreferenceMemory {
    pub fn new(storage: Arc<SqliteStorageBackend>) -> Self {
        Self { storage }
    }
}

impl Memory for PreferenceMemory {
    fn kind(&self) -> MemoryKind {
        MemoryKind::Preference
    }

    fn save(&self, ctx: &MemoryContext, record: MemoryRecord) -> Result<String, MemoryError> {
        let confidence = record
            .payload
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let source = record
            .payload
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("inferred");

        if confidence < 0.9 && source != "explicit" {
            return Err(MemoryError::Invalid(
                "preference confidence too low".into(),
            ));
        }

        let key = record
            .payload
            .get("key")
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
            r.payload.get("key").and_then(|v| v.as_str()) == Some(key)
        }) {
            let id = existing_record.id.as_deref().unwrap_or("");
            self.update(
                id,
                MemoryRecord {
                    id: Some(id.to_string()),
                    kind: MemoryKind::Preference,
                    payload: record.payload,
                    created_at: existing_record.created_at,
                    updated_at: Some(chrono_now()),
                },
            )?;
            return Ok(id.to_string());
        }

        let now = chrono_now();
        let row = MemoryRow {
            id: record.id.unwrap_or_default(),
            workspace_path: ctx.workspace_path.display().to_string(),
            created_at: now,
            updated_at: now,
            payload: record.payload.to_string(),
        };
        self.storage
            .insert(MemoryKind::Preference, &row)
            .map_err(Into::into)
    }

    fn retrieve(&self, query: &RetrieveQuery) -> Result<Vec<MemoryRecord>, MemoryError> {
        let rows = self.storage.query(StorageQuery {
            table: MemoryKind::Preference,
            workspace_path: query.workspace_path.display().to_string(),
            limit: query.limit.or(Some(50)),
            order_desc: false,
        })?;
        Ok(rows
            .into_iter()
            .map(|r| MemoryRecord {
                id: Some(r.id),
                kind: MemoryKind::Preference,
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
        self.storage.update(MemoryKind::Preference, id, &row)?;
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<(), MemoryError> {
        self.storage.delete(MemoryKind::Preference, id)?;
        Ok(())
    }

    fn summarize(&self, query: &RetrieveQuery) -> Result<String, MemoryError> {
        let records = self.retrieve(query)?;
        if records.is_empty() {
            return Ok(String::new());
        }
        let lines: Vec<String> = records
            .iter()
            .filter_map(|r| {
                let key = r.payload.get("key")?.as_str()?;
                let value = r.payload.get("value")?.as_str()?;
                Some(format!("- {key}: {value}"))
            })
            .collect();
        Ok(format!("User preferences:\n{}", lines.join("\n")))
    }
}
