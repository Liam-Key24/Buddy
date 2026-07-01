use std::sync::Arc;

use buddy_database::Database;
use rusqlite::params;
use uuid::Uuid;

use super::{MemoryRow, StorageBackend, StorageQuery};
use crate::error::StorageError;
use crate::types::{chrono_now, MemoryKind};

pub struct SqliteStorageBackend {
    db: Arc<Database>,
}

impl SqliteStorageBackend {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn db(&self) -> Arc<Database> {
        self.db.clone()
    }
}

impl StorageBackend for SqliteStorageBackend {
    fn insert(&self, table: MemoryKind, row: &MemoryRow) -> Result<String, StorageError> {
        let table_name = table.table_name();
        if table == MemoryKind::Conversation {
            return Err(StorageError::UnknownTable(
                "conversation uses messages adapter".into(),
            ));
        }
        let id = if row.id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            row.id.clone()
        };
        let insert_id = id.clone();
        let importance = row.importance.unwrap_or(default_importance(table));
        self.db.with_conn(|conn| {
            conn.execute(
                &format!(
                    "INSERT INTO {table_name} (id, workspace_path, created_at, updated_at, payload, search_text, embedding, importance) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
                ),
                params![
                    insert_id,
                    row.workspace_path,
                    row.created_at,
                    row.updated_at,
                    row.payload,
                    row.search_text,
                    row.embedding,
                    importance,
                ],
            )?;
            Ok(())
        })?;
        Ok(id)
    }

    fn query(&self, query: StorageQuery) -> Result<Vec<MemoryRow>, StorageError> {
        let table_name = query.table.table_name();
        if query.table == MemoryKind::Conversation {
            return Err(StorageError::UnknownTable(
                "conversation uses messages adapter".into(),
            ));
        }
        let limit = query.limit.unwrap_or(100);
        let order = if query.order_desc { "DESC" } else { "ASC" };
        let sql = format!(
            "SELECT id, workspace_path, created_at, updated_at, payload, search_text, embedding, importance \
             FROM {table_name} WHERE workspace_path = ?1 ORDER BY updated_at {order} LIMIT ?2"
        );
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(
                params![query.workspace_path, limit as i64],
                |row| {
                    Ok(MemoryRow {
                        id: row.get(0)?,
                        workspace_path: row.get(1)?,
                        created_at: row.get(2)?,
                        updated_at: row.get(3)?,
                        payload: row.get(4)?,
                        search_text: row.get(5)?,
                        embedding: row.get(6)?,
                        importance: row.get(7)?,
                    })
                },
            )?;
            rows.collect::<Result<Vec<_>, _>>().map_err(buddy_database::DbError::from)
        })
        .map_err(StorageError::from)
    }

    fn update(&self, table: MemoryKind, id: &str, row: &MemoryRow) -> Result<(), StorageError> {
        let table_name = table.table_name();
        self.db.with_conn(|conn| {
            conn.execute(
                &format!(
                    "UPDATE {table_name} SET payload = ?1, updated_at = ?2, search_text = ?3, embedding = ?4, importance = ?5 WHERE id = ?6"
                ),
                params![
                    row.payload,
                    chrono_now(),
                    row.search_text,
                    row.embedding,
                    row.importance,
                    id,
                ],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    fn delete(&self, table: MemoryKind, id: &str) -> Result<(), StorageError> {
        let table_name = table.table_name();
        self.db.with_conn(|conn| {
            conn.execute(
                &format!("DELETE FROM {table_name} WHERE id = ?1"),
                params![id],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    fn delete_by_workspace(&self, table: MemoryKind, workspace_path: &str) -> Result<(), StorageError> {
        let table_name = table.table_name();
        self.db.with_conn(|conn| {
            conn.execute(
                &format!("DELETE FROM {table_name} WHERE workspace_path = ?1"),
                params![workspace_path],
            )?;
            Ok(())
        })?;
        Ok(())
    }
}

pub fn default_importance(kind: MemoryKind) -> f64 {
    match kind {
        MemoryKind::Working => 1.0,
        MemoryKind::Handover => 0.9,
        MemoryKind::Decision => 0.85,
        MemoryKind::Project => 0.8,
        MemoryKind::Reflection => 0.75,
        MemoryKind::Preference => 0.7,
        MemoryKind::Error => 0.7,
        MemoryKind::Tool => 0.5,
        MemoryKind::Conversation => 0.6,
    }
}
