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
        self.db.with_conn(|conn| {
            conn.execute(
                &format!(
                    "INSERT INTO {table_name} (id, workspace_path, created_at, updated_at, payload) VALUES (?1, ?2, ?3, ?4, ?5)"
                ),
                params![insert_id, row.workspace_path, row.created_at, row.updated_at, row.payload],
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
            "SELECT id, workspace_path, created_at, updated_at, payload FROM {table_name} \
             WHERE workspace_path = ?1 ORDER BY updated_at {order} LIMIT ?2"
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
                    "UPDATE {table_name} SET payload = ?1, updated_at = ?2 WHERE id = ?3"
                ),
                params![row.payload, chrono_now(), id],
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
