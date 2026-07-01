use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

const MIGRATIONS: &[(&str, &str)] = &[
    ("001_init", include_str!("../migrations/001_init.sql")),
    ("002_memory", include_str!("../migrations/002_memory.sql")),
    (
        "003_storage",
        include_str!("../migrations/003_storage_improvements.sql"),
    ),
    (
        "004_intelligence",
        include_str!("../migrations/004_intelligence.sql"),
    ),
    (
        "005_knowledge_graph",
        include_str!("../migrations/005_knowledge_graph.sql"),
    ),
    (
        "006_learning",
        include_str!("../migrations/006_learning.sql"),
    ),
    (
        "007_workspace",
        include_str!("../migrations/007_workspace.sql"),
    ),
];

#[derive(Debug, Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("conversation not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSearchResult {
    pub message_id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                DbError::Sqlite(rusqlite::Error::InvalidPath(format!("{e}").into()))
            })?;
        }

        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_millis(5000))?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;

        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version TEXT PRIMARY KEY,
                applied_at INTEGER NOT NULL
            )",
            [],
        )?;

        let migration_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?;

        if migration_count == 0 {
            let legacy_db: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='conversations'",
                [],
                |row| row.get(0),
            )?;
            if legacy_db > 0 {
                let now = chrono_now();
                conn.execute(
                    "INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                    params!["001_init", now],
                )?;
            }
        }

        for (version, sql) in MIGRATIONS {
            let applied: i64 = conn.query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = ?1",
                params![version],
                |row| row.get(0),
            )?;
            if applied > 0 {
                if *version == "002_memory" && !Self::memory_tables_exist(&conn)? {
                    conn.execute_batch(sql)?;
                }
                continue;
            }
            conn.execute_batch(sql)?;
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![version, chrono_now()],
            )?;
        }

        Ok(())
    }

    fn memory_tables_exist(conn: &Connection) -> Result<bool, DbError> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='memory_handover'",
            [],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn with_conn<F, T>(&self, f: F) -> Result<T, DbError>
    where
        F: FnOnce(&Connection) -> Result<T, DbError>,
    {
        let conn = self.conn.lock().unwrap();
        f(&conn)
    }

    #[instrument(skip(self))]
    pub fn list_conversations(&self) -> Result<Vec<Conversation>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at FROM conversations ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn create_conversation(&self, title: &str) -> Result<Conversation, DbError> {
        let now = chrono_now();
        let conv = Conversation {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            created_at: now,
            updated_at: now,
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![conv.id, conv.title, conv.created_at, conv.updated_at],
        )?;
        Ok(conv)
    }

    pub fn delete_conversation(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(DbError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn get_conversation(&self, id: &str) -> Result<Conversation, DbError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, title, created_at, updated_at FROM conversations WHERE id = ?1",
            params![id],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(id.to_string()),
            other => DbError::from(other),
        })
    }

    pub fn update_conversation_title(&self, id: &str, title: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute(
            "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, chrono_now(), id],
        )?;
        if affected == 0 {
            return Err(DbError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn touch_conversation(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![chrono_now(), id],
        )?;
        Ok(())
    }

    pub fn get_messages(&self, conversation_id: &str) -> Result<Vec<Message>, DbError> {
        self.get_messages_paginated(conversation_id, None, None)
    }

    pub fn get_messages_paginated(
        &self,
        conversation_id: &str,
        limit: Option<usize>,
        before_created_at: Option<i64>,
    ) -> Result<Vec<Message>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut sql = String::from(
            "SELECT id, conversation_id, role, content, created_at, metadata \
             FROM messages WHERE conversation_id = ?1",
        );
        if before_created_at.is_some() {
            sql.push_str(" AND created_at < ?2");
        }
        sql.push_str(" ORDER BY created_at ASC");
        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let mut stmt = conn.prepare(&sql)?;
        let map_row = |row: &rusqlite::Row<'_>| {
            Ok(Message {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                metadata: row.get(5)?,
            })
        };

        let rows = if let Some(before) = before_created_at {
            stmt.query_map(params![conversation_id, before], map_row)?
        } else {
            stmt.query_map(params![conversation_id], map_row)?
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn add_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
    ) -> Result<Message, DbError> {
        self.add_message_with_metadata(conversation_id, role, content, None)
    }

    pub fn add_message_with_metadata(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
        metadata: Option<&str>,
    ) -> Result<Message, DbError> {
        let msg = Message {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            created_at: chrono_now(),
            metadata: metadata.map(str::to_string),
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, created_at, metadata) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                msg.id,
                msg.conversation_id,
                msg.role,
                msg.content,
                msg.created_at,
                msg.metadata,
            ],
        )?;
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![msg.created_at, conversation_id],
        )?;
        Ok(msg)
    }

    pub fn search_messages(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MessageSearchResult>, DbError> {
        let conn = self.conn.lock().unwrap();
        let fts_query = query
            .split_whitespace()
            .map(|term| format!("\"{term}\"*"))
            .collect::<Vec<_>>()
            .join(" AND ");

        if fts_query.is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = conn.prepare(
            "SELECT m.id, m.conversation_id, m.role, m.content, m.created_at \
             FROM messages_fts fts \
             JOIN messages m ON m.rowid = fts.rowid \
             WHERE messages_fts MATCH ?1 \
             ORDER BY rank \
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![fts_query, limit as i64], |row| {
            Ok(MessageSearchResult {
                message_id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_all_settings(&self) -> Result<Vec<(String, String)>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }
}

pub fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::fs;

    #[test]
    fn migrations_apply_and_support_metadata_search() {
        let dir = std::env::temp_dir().join(format!("buddy-db-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("buddy.db");

        let db = Database::open(&path).unwrap();
        let conv = db.create_conversation("test").unwrap();
        db.add_message_with_metadata(
            &conv.id,
            "user",
            "hello sqlite storage",
            None,
        )
        .unwrap();
        db.add_message_with_metadata(
            &conv.id,
            "assistant",
            "storage improved",
            Some(r#"{"intent":"respond"}"#),
        )
        .unwrap();

        let messages = db.get_messages(&conv.id).unwrap();
        assert_eq!(messages.len(), 2);
        assert!(messages[1].metadata.is_some());

        let hits = db.search_messages("sqlite", 10).unwrap();
        assert!(!hits.is_empty());

        let versions: Vec<String> = db
            .with_conn(|conn| {
                let mut stmt = conn.prepare("SELECT version FROM schema_migrations ORDER BY version")?;
                let rows = stmt.query_map([], |row| row.get(0))?;
                rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
            })
            .unwrap();
        assert!(versions.contains(&"003_storage".to_string()));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_db_repairs_missing_memory_tables() {
        let dir = std::env::temp_dir().join(format!("buddy-db-legacy-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("buddy.db");

        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(include_str!("../migrations/001_init.sql"))
                .unwrap();
            conn.execute(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version TEXT PRIMARY KEY,
                    applied_at INTEGER NOT NULL
                )",
                [],
            )
            .unwrap();
            let now = chrono_now();
            for version in ["001_init", "002_memory"] {
                conn.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                    params![version, now],
                )
                .unwrap();
            }
        }

        let db = Database::open(&path).unwrap();
        let handover_exists: i64 = db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='memory_handover'",
                    [],
                    |row| row.get(0),
                )
                .map_err(DbError::from)
            })
            .unwrap();
        assert_eq!(handover_exists, 1);

        let _ = fs::remove_dir_all(dir);
    }
}
