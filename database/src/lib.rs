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
    ("008_sparks", include_str!("../migrations/008_sparks.sql")),
    (
        "009_agents_and_audit",
        include_str!("../migrations/009_agents_and_audit.sql"),
    ),
    (
        "010_calendar",
        include_str!("../migrations/010_calendar.sql"),
    ),
    (
        "011_calendar_columns",
        include_str!("../migrations/011_calendar_columns.sql"),
    ),
    (
        "012_buddy_calendar",
        include_str!("../migrations/012_buddy_calendar.sql"),
    ),
    (
        "013_lifestyle_schedule",
        include_str!("../migrations/013_lifestyle_schedule.sql"),
    ),
    (
        "014_memory_runtime_state",
        include_str!("../migrations/014_memory_runtime_state.sql"),
    ),
];

pub const SPARK_STALE_AGE_MS: i64 = 30 * 24 * 60 * 60 * 1000;
pub const SPARK_NUDGE_COOLDOWN_MS: i64 = 7 * 24 * 60 * 60 * 1000;

pub const SPARK_TAGS: &[&str] = &[
    "projects",
    "the_land",
    "the_van",
    "general_life",
    "travelling",
];

pub fn validate_spark_tags(tags: &[String]) -> Result<Vec<String>, DbError> {
    if tags.is_empty() {
        return Err(DbError::Sqlite(rusqlite::Error::InvalidParameterName(
            "at least one tag required".into(),
        )));
    }
    let mut validated = Vec::new();
    for tag in tags {
        if SPARK_TAGS.contains(&tag.as_str()) {
            if !validated.contains(tag) {
                validated.push(tag.clone());
            }
        } else {
            return Err(DbError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!("invalid tag: {tag}").into(),
            )));
        }
    }
    Ok(validated)
}

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
    #[serde(default = "default_conversation_kind")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_path: Option<String>,
}

fn default_conversation_kind() -> String {
    "buddy".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalAction {
    pub id: String,
    pub action_type: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail_json: Option<String>,
    pub approved: bool,
    pub created_at: i64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spark {
    pub id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_nudged_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_conversation_id: Option<String>,
}

/// Native BUDDY Calendar event (source of truth).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuddyCalendarEventRow {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub category: String,
    pub color: Option<String>,
    pub start_time: i64,
    pub end_time: i64,
    pub all_day: bool,
    pub timezone: String,
    pub recurrence_json: Option<String>,
    pub reminders_json: String,
    pub external_provider: Option<String>,
    pub external_event_id: Option<String>,
    pub sync_status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Scheduled reminder delivery state for an event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderStateRow {
    pub id: String,
    pub event_id: String,
    pub reminder_minutes: i64,
    pub fire_at: i64,
    pub status: String,
    pub snoozed_until: Option<i64>,
    pub delivered_at: Option<i64>,
    pub created_at: i64,
}

/// Work or sleep schedule rule (segments expanded at read time).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifestyleScheduleRuleRow {
    pub kind: String,
    pub segments_json: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamEntryRow {
    pub id: String,
    pub sleep_date: String,
    pub title: Option<String>,
    pub body: String,
    pub tags_json: String,
    pub mood: Option<i64>,
    pub sleep_quality: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkDayLogRow {
    pub work_date: String,
    pub actual_start_ms: Option<i64>,
    pub actual_end_ms: Option<i64>,
    pub sales_amount: f64,
    pub sales_currency: String,
    pub notes: Option<String>,
    pub updated_at: i64,
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
        self.list_conversations_by_kind(None)
    }

    pub fn list_conversations_by_kind(
        &self,
        kind: Option<&str>,
    ) -> Result<Vec<Conversation>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut sql = String::from(
            "SELECT id, title, created_at, updated_at, kind, focus_mode, workspace_path \
             FROM conversations",
        );
        if kind.is_some() {
            sql.push_str(" WHERE kind = ?1");
        }
        sql.push_str(" ORDER BY updated_at DESC");
        let mut stmt = conn.prepare(&sql)?;
        let map_row = |row: &rusqlite::Row<'_>| {
            Ok(Conversation {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                kind: row.get(4)?,
                focus_mode: row.get(5)?,
                workspace_path: row.get(6)?,
            })
        };
        let rows = if let Some(kind) = kind {
            stmt.query_map(params![kind], map_row)?
        } else {
            stmt.query_map([], map_row)?
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn create_conversation(&self, title: &str) -> Result<Conversation, DbError> {
        self.create_conversation_with_kind(title, "buddy", None, None)
    }

    pub fn create_conversation_with_kind(
        &self,
        title: &str,
        kind: &str,
        focus_mode: Option<&str>,
        workspace_path: Option<&str>,
    ) -> Result<Conversation, DbError> {
        let now = chrono_now();
        let conv = Conversation {
            id: Uuid::new_v4().to_string(),
            title: title.to_string(),
            created_at: now,
            updated_at: now,
            kind: kind.to_string(),
            focus_mode: focus_mode.map(str::to_string),
            workspace_path: workspace_path.map(str::to_string),
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO conversations (id, title, created_at, updated_at, kind, focus_mode, workspace_path) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                conv.id,
                conv.title,
                conv.created_at,
                conv.updated_at,
                conv.kind,
                conv.focus_mode,
                conv.workspace_path
            ],
        )?;
        Ok(conv)
    }

    pub fn set_conversation_focus(&self, id: &str, focus_mode: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute(
            "UPDATE conversations SET focus_mode = ?1, updated_at = ?2 WHERE id = ?3",
            params![focus_mode, chrono_now(), id],
        )?;
        if affected == 0 {
            return Err(DbError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn set_conversation_workspace(
        &self,
        id: &str,
        workspace_path: &str,
    ) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute(
            "UPDATE conversations SET workspace_path = ?1, updated_at = ?2 WHERE id = ?3",
            params![workspace_path, chrono_now(), id],
        )?;
        if affected == 0 {
            return Err(DbError::NotFound(id.to_string()));
        }
        Ok(())
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
            "SELECT id, title, created_at, updated_at, kind, focus_mode, workspace_path \
             FROM conversations WHERE id = ?1",
            params![id],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    kind: row.get(4)?,
                    focus_mode: row.get(5)?,
                    workspace_path: row.get(6)?,
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

    /// Convenience wrapper for the common "read a setting, fall back to a
    /// default" pattern used throughout plugins and command handlers.
    pub fn get_setting_or(&self, key: &str, default: &str) -> String {
        self.get_setting(key)
            .ok()
            .flatten()
            .unwrap_or_else(|| default.to_string())
    }

    /// Memory-owned temporary working state (pending clarification, etc.).
    pub fn get_runtime_state(&self, key: &str) -> Result<Option<String>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT value FROM memory_runtime_state WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_runtime_state(&self, key: &str, value: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        let now = chrono_now();
        conn.execute(
            "INSERT INTO memory_runtime_state (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now],
        )?;
        Ok(())
    }

    pub fn delete_runtime_state(&self, key: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM memory_runtime_state WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn get_all_settings(&self) -> Result<Vec<(String, String)>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn log_external_action(
        &self,
        action_type: &str,
        summary: &str,
        detail_json: Option<&str>,
        approved: bool,
    ) -> Result<ExternalAction, DbError> {
        let action = ExternalAction {
            id: Uuid::new_v4().to_string(),
            action_type: action_type.to_string(),
            summary: summary.to_string(),
            detail_json: detail_json.map(str::to_string),
            approved,
            created_at: chrono_now(),
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO external_actions (id, action_type, summary, detail_json, approved, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                action.id,
                action.action_type,
                action.summary,
                action.detail_json,
                action.approved as i64,
                action.created_at
            ],
        )?;
        Ok(action)
    }

    pub fn list_external_actions(&self, limit: usize) -> Result<Vec<ExternalAction>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, action_type, summary, detail_json, approved, created_at \
             FROM external_actions ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let approved: i64 = row.get(4)?;
            Ok(ExternalAction {
                id: row.get(0)?,
                action_type: row.get(1)?,
                summary: row.get(2)?,
                detail_json: row.get(3)?,
                approved: approved != 0,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn log_file_operation(
        &self,
        path: &str,
        op: &str,
        conversation_id: Option<&str>,
    ) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO file_operations (id, path, op, conversation_id, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                Uuid::new_v4().to_string(),
                path,
                op,
                conversation_id,
                chrono_now()
            ],
        )?;
        Ok(())
    }

    fn row_to_spark(row: &rusqlite::Row<'_>) -> Result<Spark, rusqlite::Error> {
        let tags_json: String = row.get(2)?;
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        Ok(Spark {
            id: row.get(0)?,
            content: row.get(1)?,
            tags,
            status: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
            last_nudged_at: row.get(6)?,
            source_conversation_id: row.get(7)?,
        })
    }

    pub fn create_spark(
        &self,
        content: &str,
        tags: &[String],
        source_conversation_id: Option<&str>,
    ) -> Result<Spark, DbError> {
        let tags = validate_spark_tags(tags)?;
        let now = chrono_now();
        let spark = Spark {
            id: Uuid::new_v4().to_string(),
            content: content.trim().to_string(),
            tags,
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
            last_nudged_at: None,
            source_conversation_id: source_conversation_id.map(str::to_string),
        };
        let tags_json = serde_json::to_string(&spark.tags).map_err(|e| {
            DbError::Sqlite(rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sparks (id, content, tags, status, created_at, updated_at, last_nudged_at, source_conversation_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                spark.id,
                spark.content,
                tags_json,
                spark.status,
                spark.created_at,
                spark.updated_at,
                spark.last_nudged_at,
                spark.source_conversation_id,
            ],
        )?;
        Ok(spark)
    }

    pub fn list_sparks(&self, status: Option<&str>) -> Result<Vec<Spark>, DbError> {
        let conn = self.conn.lock().unwrap();
        let (sql, use_status) = match status {
            Some(_) => (
                "SELECT id, content, tags, status, created_at, updated_at, last_nudged_at, source_conversation_id
                 FROM sparks WHERE status = ?1 ORDER BY updated_at DESC",
                true,
            ),
            None => (
                "SELECT id, content, tags, status, created_at, updated_at, last_nudged_at, source_conversation_id
                 FROM sparks ORDER BY updated_at DESC",
                false,
            ),
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = if use_status {
            stmt.query_map(params![status.unwrap()], Self::row_to_spark)?
        } else {
            stmt.query_map([], Self::row_to_spark)?
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn get_spark(&self, id: &str) -> Result<Spark, DbError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, content, tags, status, created_at, updated_at, last_nudged_at, source_conversation_id
             FROM sparks WHERE id = ?1",
            params![id],
            Self::row_to_spark,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(id.to_string()),
            other => DbError::from(other),
        })
    }

    pub fn get_stale_sparks(
        &self,
        age_ms: i64,
        nudge_cooldown_ms: i64,
    ) -> Result<Vec<Spark>, DbError> {
        let now = chrono_now();
        let stale_before = now - age_ms;
        let nudge_before = now - nudge_cooldown_ms;
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, content, tags, status, created_at, updated_at, last_nudged_at, source_conversation_id
             FROM sparks
             WHERE status = 'active'
               AND updated_at < ?1
               AND (last_nudged_at IS NULL OR last_nudged_at < ?2)
             ORDER BY updated_at ASC",
        )?;
        let rows = stmt.query_map(params![stale_before, nudge_before], Self::row_to_spark)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn count_stale_sparks(&self, age_ms: i64, nudge_cooldown_ms: i64) -> Result<i64, DbError> {
        let now = chrono_now();
        let stale_before = now - age_ms;
        let nudge_before = now - nudge_cooldown_ms;
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM sparks
             WHERE status = 'active'
               AND updated_at < ?1
               AND (last_nudged_at IS NULL OR last_nudged_at < ?2)",
            params![stale_before, nudge_before],
            |row| row.get(0),
        )
        .map_err(DbError::from)
    }

    pub fn update_spark(
        &self,
        id: &str,
        action: &str,
        content: Option<&str>,
        tags: Option<&[String]>,
    ) -> Result<Spark, DbError> {
        let mut spark = self.get_spark(id)?;
        let now = chrono_now();

        match action {
            "respark" => {
                if let Some(c) = content {
                    spark.content = c.trim().to_string();
                }
                if let Some(t) = tags {
                    spark.tags = validate_spark_tags(t)?;
                }
                spark.updated_at = now;
                spark.status = "active".to_string();
            }
            "archive" => {
                spark.status = "archived".to_string();
                spark.updated_at = now;
            }
            "edit" => {
                if let Some(c) = content {
                    spark.content = c.trim().to_string();
                }
                if let Some(t) = tags {
                    spark.tags = validate_spark_tags(t)?;
                }
            }
            other => {
                return Err(DbError::Sqlite(rusqlite::Error::InvalidParameterName(
                    format!("unknown action: {other}").into(),
                )));
            }
        }

        let tags_json = serde_json::to_string(&spark.tags).map_err(|e| {
            DbError::Sqlite(rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute(
            "UPDATE sparks SET content = ?1, tags = ?2, status = ?3, updated_at = ?4 WHERE id = ?5",
            params![spark.content, tags_json, spark.status, spark.updated_at, spark.id],
        )?;
        if affected == 0 {
            return Err(DbError::NotFound(id.to_string()));
        }
        Ok(spark)
    }

    pub fn mark_sparks_nudged(&self, ids: &[String]) -> Result<(), DbError> {
        if ids.is_empty() {
            return Ok(());
        }
        let now = chrono_now();
        let conn = self.conn.lock().unwrap();
        for id in ids {
            conn.execute(
                "UPDATE sparks SET last_nudged_at = ?1 WHERE id = ?2",
                params![now, id],
            )?;
        }
        Ok(())
    }

    pub fn delete_spark(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM sparks WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(DbError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn format_stale_sparks_context(sparks: &[Spark]) -> String {
        sparks
            .iter()
            .map(|s| {
                let tags = s.tags.join(", ");
                format!("- [{}] (id: {}) {}", tags, s.id, s.content)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn row_to_buddy_calendar_event(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<BuddyCalendarEventRow> {
        let all_day: i64 = row.get(8)?;
        Ok(BuddyCalendarEventRow {
            id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            location: row.get(3)?,
            category: row.get(4)?,
            color: row.get(5)?,
            start_time: row.get(6)?,
            end_time: row.get(7)?,
            all_day: all_day != 0,
            timezone: row.get(9)?,
            recurrence_json: row.get(10)?,
            reminders_json: row.get(11)?,
            external_provider: row.get(12)?,
            external_event_id: row.get(13)?,
            sync_status: row.get(14)?,
            created_at: row.get(15)?,
            updated_at: row.get(16)?,
        })
    }

    fn row_to_reminder_state(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReminderStateRow> {
        Ok(ReminderStateRow {
            id: row.get(0)?,
            event_id: row.get(1)?,
            reminder_minutes: row.get(2)?,
            fire_at: row.get(3)?,
            status: row.get(4)?,
            snoozed_until: row.get(5)?,
            delivered_at: row.get(6)?,
            created_at: row.get(7)?,
        })
    }

    const BUDDY_CALENDAR_SELECT: &'static str = "SELECT id, title, description, location, category,
            color, start_time, end_time, all_day, timezone, recurrence_json, reminders_json,
            external_provider, external_event_id, sync_status, created_at, updated_at
         FROM buddy_calendar_events";

    const REMINDER_SELECT: &'static str = "SELECT id, event_id, reminder_minutes, fire_at, status,
            snoozed_until, delivered_at, created_at
         FROM calendar_reminder_states";

    pub fn upsert_buddy_calendar_event(
        &self,
        event: &BuddyCalendarEventRow,
    ) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO buddy_calendar_events (
                id, title, description, location, category, color,
                start_time, end_time, all_day, timezone, recurrence_json, reminders_json,
                external_provider, external_event_id, sync_status, created_at, updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
             )
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                description = excluded.description,
                location = excluded.location,
                category = excluded.category,
                color = excluded.color,
                start_time = excluded.start_time,
                end_time = excluded.end_time,
                all_day = excluded.all_day,
                timezone = excluded.timezone,
                recurrence_json = excluded.recurrence_json,
                reminders_json = excluded.reminders_json,
                external_provider = excluded.external_provider,
                external_event_id = excluded.external_event_id,
                sync_status = excluded.sync_status,
                updated_at = excluded.updated_at",
            params![
                event.id,
                event.title,
                event.description,
                event.location,
                event.category,
                event.color,
                event.start_time,
                event.end_time,
                event.all_day as i64,
                event.timezone,
                event.recurrence_json,
                event.reminders_json,
                event.external_provider,
                event.external_event_id,
                event.sync_status,
                event.created_at,
                event.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_buddy_calendar_event(&self, id: &str) -> Result<BuddyCalendarEventRow, DbError> {
        let conn = self.conn.lock().unwrap();
        let sql = format!("{} WHERE id = ?1", Self::BUDDY_CALENDAR_SELECT);
        conn.query_row(&sql, params![id], Self::row_to_buddy_calendar_event)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(id.to_string()),
                other => DbError::from(other),
            })
    }

    pub fn list_buddy_calendar_events(
        &self,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<BuddyCalendarEventRow>, DbError> {
        let conn = self.conn.lock().unwrap();
        // Include recurring masters that may expand into the range (start before end).
        let sql = format!(
            "{} WHERE (start_time < ?2 AND end_time > ?1)
                OR (recurrence_json IS NOT NULL AND start_time < ?2)
             ORDER BY start_time ASC",
            Self::BUDDY_CALENDAR_SELECT
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![start_ms, end_ms], Self::row_to_buddy_calendar_event)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn search_buddy_calendar_events(
        &self,
        query: &str,
        start_ms: Option<i64>,
        end_ms: Option<i64>,
    ) -> Result<Vec<BuddyCalendarEventRow>, DbError> {
        let pattern = format!("%{}%", query.trim());
        let conn = self.conn.lock().unwrap();
        match (start_ms, end_ms) {
            (Some(start), Some(end)) => {
                let sql = format!(
                    "{} WHERE (title LIKE ?1 OR IFNULL(description, '') LIKE ?1 OR IFNULL(location, '') LIKE ?1)
                     AND start_time < ?3 AND end_time > ?2
                     ORDER BY start_time ASC",
                    Self::BUDDY_CALENDAR_SELECT
                );
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(
                    params![pattern, start, end],
                    Self::row_to_buddy_calendar_event,
                )?;
                rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
            }
            _ => {
                let sql = format!(
                    "{} WHERE title LIKE ?1 OR IFNULL(description, '') LIKE ?1 OR IFNULL(location, '') LIKE ?1
                     ORDER BY start_time ASC",
                    Self::BUDDY_CALENDAR_SELECT
                );
                let mut stmt = conn.prepare(&sql)?;
                let rows =
                    stmt.query_map(params![pattern], Self::row_to_buddy_calendar_event)?;
                rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
            }
        }
    }

    pub fn delete_buddy_calendar_event(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        let affected =
            conn.execute("DELETE FROM buddy_calendar_events WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(DbError::NotFound(id.to_string()));
        }
        let _ = conn.execute(
            "DELETE FROM calendar_reminder_states WHERE event_id = ?1",
            params![id],
        );
        Ok(())
    }

    pub fn delete_all_buddy_calendar_events(&self) -> Result<usize, DbError> {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute("DELETE FROM calendar_reminder_states", [])?;
        let affected = conn.execute("DELETE FROM buddy_calendar_events", [])?;
        Ok(affected)
    }

    // --- Lifestyle schedule / dreams / work logs ---

    pub fn list_lifestyle_schedule_rules(&self) -> Result<Vec<LifestyleScheduleRuleRow>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT kind, segments_json, updated_at FROM lifestyle_schedule_rules ORDER BY kind",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(LifestyleScheduleRuleRow {
                kind: row.get(0)?,
                segments_json: row.get(1)?,
                updated_at: row.get(2)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn get_lifestyle_schedule_rule(
        &self,
        kind: &str,
    ) -> Result<LifestyleScheduleRuleRow, DbError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT kind, segments_json, updated_at FROM lifestyle_schedule_rules WHERE kind = ?1",
            params![kind],
            |row| {
                Ok(LifestyleScheduleRuleRow {
                    kind: row.get(0)?,
                    segments_json: row.get(1)?,
                    updated_at: row.get(2)?,
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(kind.to_string()),
            other => DbError::from(other),
        })
    }

    pub fn upsert_dream_entry(&self, row: &DreamEntryRow) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO dream_entries (
                id, sleep_date, title, body, tags_json, mood, sleep_quality, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                sleep_date = excluded.sleep_date,
                title = excluded.title,
                body = excluded.body,
                tags_json = excluded.tags_json,
                mood = excluded.mood,
                sleep_quality = excluded.sleep_quality,
                updated_at = excluded.updated_at",
            params![
                row.id,
                row.sleep_date,
                row.title,
                row.body,
                row.tags_json,
                row.mood,
                row.sleep_quality,
                row.created_at,
                row.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_dream_entry(&self, id: &str) -> Result<DreamEntryRow, DbError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, sleep_date, title, body, tags_json, mood, sleep_quality, created_at, updated_at
             FROM dream_entries WHERE id = ?1",
            params![id],
            Self::row_to_dream_entry,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(id.to_string()),
            other => DbError::from(other),
        })
    }

    pub fn list_dreams_for_date(&self, sleep_date: &str) -> Result<Vec<DreamEntryRow>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, sleep_date, title, body, tags_json, mood, sleep_quality, created_at, updated_at
             FROM dream_entries WHERE sleep_date = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![sleep_date], Self::row_to_dream_entry)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn search_dream_entries(&self, query: &str) -> Result<Vec<DreamEntryRow>, DbError> {
        let pattern = format!("%{}%", query.trim());
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, sleep_date, title, body, tags_json, mood, sleep_quality, created_at, updated_at
             FROM dream_entries
             WHERE body LIKE ?1 OR IFNULL(title, '') LIKE ?1 OR tags_json LIKE ?1
             ORDER BY sleep_date DESC, created_at DESC",
        )?;
        let rows = stmt.query_map(params![pattern], Self::row_to_dream_entry)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn delete_dream_entry(&self, id: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM dream_entries WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(DbError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn get_work_day_log(&self, work_date: &str) -> Result<Option<WorkDayLogRow>, DbError> {
        let conn = self.conn.lock().unwrap();
        match conn.query_row(
            "SELECT work_date, actual_start_ms, actual_end_ms, sales_amount, sales_currency, notes, updated_at
             FROM work_day_logs WHERE work_date = ?1",
            params![work_date],
            Self::row_to_work_day_log,
        ) {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DbError::from(e)),
        }
    }

    pub fn list_work_day_logs(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<WorkDayLogRow>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT work_date, actual_start_ms, actual_end_ms, sales_amount, sales_currency, notes, updated_at
             FROM work_day_logs
             WHERE work_date >= ?1 AND work_date <= ?2
             ORDER BY work_date ASC",
        )?;
        let rows = stmt.query_map(params![start_date, end_date], Self::row_to_work_day_log)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn upsert_work_day_log(&self, row: &WorkDayLogRow) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO work_day_logs (
                work_date, actual_start_ms, actual_end_ms, sales_amount, sales_currency, notes, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(work_date) DO UPDATE SET
                actual_start_ms = excluded.actual_start_ms,
                actual_end_ms = excluded.actual_end_ms,
                sales_amount = excluded.sales_amount,
                sales_currency = excluded.sales_currency,
                notes = excluded.notes,
                updated_at = excluded.updated_at",
            params![
                row.work_date,
                row.actual_start_ms,
                row.actual_end_ms,
                row.sales_amount,
                row.sales_currency,
                row.notes,
                row.updated_at,
            ],
        )?;
        Ok(())
    }

    fn row_to_dream_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<DreamEntryRow> {
        Ok(DreamEntryRow {
            id: row.get(0)?,
            sleep_date: row.get(1)?,
            title: row.get(2)?,
            body: row.get(3)?,
            tags_json: row.get(4)?,
            mood: row.get(5)?,
            sleep_quality: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    }

    fn row_to_work_day_log(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkDayLogRow> {
        Ok(WorkDayLogRow {
            work_date: row.get(0)?,
            actual_start_ms: row.get(1)?,
            actual_end_ms: row.get(2)?,
            sales_amount: row.get(3)?,
            sales_currency: row.get(4)?,
            notes: row.get(5)?,
            updated_at: row.get(6)?,
        })
    }

    pub fn upsert_reminder_state(&self, row: &ReminderStateRow) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO calendar_reminder_states (
                id, event_id, reminder_minutes, fire_at, status,
                snoozed_until, delivered_at, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                reminder_minutes = excluded.reminder_minutes,
                fire_at = excluded.fire_at,
                status = excluded.status,
                snoozed_until = excluded.snoozed_until,
                delivered_at = excluded.delivered_at",
            params![
                row.id,
                row.event_id,
                row.reminder_minutes,
                row.fire_at,
                row.status,
                row.snoozed_until,
                row.delivered_at,
                row.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete_reminder_states_for_event(&self, event_id: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM calendar_reminder_states WHERE event_id = ?1",
            params![event_id],
        )?;
        Ok(())
    }

    pub fn list_due_reminders(&self, now_ms: i64) -> Result<Vec<ReminderStateRow>, DbError> {
        let conn = self.conn.lock().unwrap();
        let sql = format!(
            "{} WHERE (status = 'pending' AND fire_at <= ?1)
                OR (status = 'snoozed' AND IFNULL(snoozed_until, 0) <= ?1)
             ORDER BY fire_at ASC",
            Self::REMINDER_SELECT
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![now_ms], Self::row_to_reminder_state)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn get_reminder_state(&self, id: &str) -> Result<ReminderStateRow, DbError> {
        let conn = self.conn.lock().unwrap();
        let sql = format!("{} WHERE id = ?1", Self::REMINDER_SELECT);
        conn.query_row(&sql, params![id], Self::row_to_reminder_state)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(id.to_string()),
                other => DbError::from(other),
            })
    }

    pub fn update_reminder_status(
        &self,
        id: &str,
        status: &str,
        snoozed_until: Option<i64>,
        delivered_at: Option<i64>,
    ) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute(
            "UPDATE calendar_reminder_states
             SET status = ?2, snoozed_until = ?3, delivered_at = ?4
             WHERE id = ?1",
            params![id, status, snoozed_until, delivered_at],
        )?;
        if affected == 0 {
            return Err(DbError::NotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn snooze_reminder(&self, id: &str, snoozed_until: i64) -> Result<(), DbError> {
        self.update_reminder_status(id, "snoozed", Some(snoozed_until), None)
    }

    pub fn dismiss_reminder(&self, id: &str) -> Result<(), DbError> {
        self.update_reminder_status(id, "dismissed", None, Some(chrono_now()))
    }

    pub fn list_active_notifications(&self) -> Result<Vec<ReminderStateRow>, DbError> {
        let conn = self.conn.lock().unwrap();
        let sql = format!(
            "{} WHERE status IN ('sent', 'snoozed')
             ORDER BY COALESCE(delivered_at, fire_at) DESC
             LIMIT 50",
            Self::REMINDER_SELECT
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], Self::row_to_reminder_state)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn count_pending_reminders(&self) -> Result<i64, DbError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM calendar_reminder_states
             WHERE status IN ('sent', 'snoozed')",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
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
