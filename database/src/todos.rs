use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{chrono_now, Database, DbError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    pub priority: String,
    pub status: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub recurrence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpsertTodo {
    #[serde(default)]
    pub id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub deadline: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub recurrence: Option<String>,
}

pub fn todo_is_overdue(todo: &Todo, today: &str) -> bool {
    if todo.status == "completed" {
        return false;
    }
    match todo.deadline.as_deref() {
        Some(d) if !d.is_empty() => d < today,
        _ => false,
    }
}

pub fn next_deadline(deadline: &str, recurrence: &str) -> Option<String> {
    let parts: Vec<i32> = deadline.split('-').filter_map(|p| p.parse().ok()).collect();
    if parts.len() != 3 {
        return None;
    }
    let (y, m, d) = (parts[0], parts[1], parts[2]);
    let ordinal = ymd_to_ordinal(y, m, d)?;
    let next = match recurrence {
        "daily" => ordinal + 1,
        "weekly" => ordinal + 7,
        "monthly" => {
            let mut nm = m + 1;
            let mut ny = y;
            if nm > 12 {
                nm = 1;
                ny += 1;
            }
            return Some(format!("{ny:04}-{nm:02}-{d:02}"));
        }
        _ => return None,
    };
    ordinal_to_ymd(next).map(|(ny, nm, nd)| format!("{ny:04}-{nm:02}-{nd:02}"))
}

pub fn ymd_to_ordinal_pub(y: i32, m: i32, d: i32) -> Option<i32> {
    ymd_to_ordinal(y, m, d)
}

pub fn add_days_to_date(date: &str, days: i64) -> Option<String> {
    let parts: Vec<i32> = date.split('-').filter_map(|p| p.parse().ok()).collect();
    if parts.len() != 3 {
        return None;
    }
    let ordinal = ymd_to_ordinal(parts[0], parts[1], parts[2])?;
    ordinal_to_ymd(ordinal + days as i32).map(|(y, m, d)| format!("{y:04}-{m:02}-{d:02}"))
}

fn ymd_to_ordinal(y: i32, m: i32, d: i32) -> Option<i32> {
    if y < 1970 || y > 2100 || m < 1 || m > 12 || d < 1 || d > 31 {
        return None;
    }
    let mut days = 0;
    for year in 1970..y {
        days += if is_leap(year) { 366 } else { 365 };
    }
    const MD: [i32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for month in 1..m {
        days += MD[(month - 1) as usize];
        if month == 2 && is_leap(y) {
            days += 1;
        }
    }
    Some(days + d)
}

fn ordinal_to_ymd(mut ordinal: i32) -> Option<(i32, i32, i32)> {
    if ordinal < 1 {
        return None;
    }
    let mut y = 1970;
    let mut years = 0;
    loop {
        if years > 200 {
            return None;
        }
        let len = if is_leap(y) { 366 } else { 365 };
        if ordinal <= len {
            break;
        }
        ordinal -= len;
        y += 1;
        years += 1;
    }
    const MD: [i32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 1..=12 {
        let mut dim = MD[(m - 1) as usize];
        if m == 2 && is_leap(y) {
            dim += 1;
        }
        if ordinal <= dim {
            return Some((y, m, ordinal));
        }
        ordinal -= dim;
    }
    None
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

impl Database {
    fn row_to_todo(row: &rusqlite::Row<'_>) -> Result<Todo, rusqlite::Error> {
        Ok(Todo {
            id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            deadline: row.get(3)?,
            priority: row.get(4)?,
            status: row.get(5)?,
            category: row.get(6)?,
            notes: row.get(7)?,
            recurrence: row.get(8)?,
            completed_at: row.get(9)?,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    }

    pub fn list_todos(
        &self,
        status: Option<&str>,
        category: Option<&str>,
    ) -> Result<Vec<Todo>, DbError> {
        self.with_conn(|conn| {
            let mut sql = String::from(
                "SELECT id, title, description, deadline, priority, status, category, notes, recurrence, completed_at, created_at, updated_at FROM todos WHERE 1=1",
            );
            if status.is_some() {
                sql.push_str(" AND status = ?1");
            }
            if category.is_some() {
                sql.push_str(if status.is_some() {
                    " AND category = ?2"
                } else {
                    " AND category = ?1"
                });
            }
            sql.push_str(" ORDER BY CASE status WHEN 'completed' THEN 1 ELSE 0 END, deadline IS NULL, deadline ASC, CASE priority WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END, updated_at DESC LIMIT 2000");
            let mut stmt = conn.prepare(&sql)?;
            let rows = match (status, category) {
                (Some(s), Some(c)) => stmt.query_map(params![s, c], Self::row_to_todo)?,
                (Some(s), None) => stmt.query_map(params![s], Self::row_to_todo)?,
                (None, Some(c)) => stmt.query_map(params![c], Self::row_to_todo)?,
                (None, None) => stmt.query_map([], Self::row_to_todo)?,
            };
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn get_todo(&self, id: &str) -> Result<Todo, DbError> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT id, title, description, deadline, priority, status, category, notes, recurrence, completed_at, created_at, updated_at FROM todos WHERE id = ?1",
                params![id],
                Self::row_to_todo,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(id.to_string()),
                other => DbError::from(other),
            })
        })
    }

    pub fn upsert_todo(&self, input: UpsertTodo) -> Result<Todo, DbError> {
        let now = chrono_now();
        let id = input
            .id
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let existing = self.get_todo(&id).ok();
        let created_at = existing.as_ref().map(|t| t.created_at).unwrap_or(now);
        let status = input
            .status
            .or_else(|| existing.as_ref().map(|t| t.status.clone()))
            .unwrap_or_else(|| "not_started".into());
        let completed_at = if status == "completed" {
            existing
                .as_ref()
                .and_then(|t| t.completed_at)
                .or(Some(now))
        } else {
            None
        };
        let todo = Todo {
            id: id.clone(),
            title: input.title.trim().to_string(),
            description: input.description.or_else(|| {
                existing.as_ref().and_then(|t| t.description.clone())
            }),
            deadline: input
                .deadline
                .or_else(|| existing.as_ref().and_then(|t| t.deadline.clone())),
            priority: input
                .priority
                .or_else(|| existing.as_ref().map(|t| t.priority.clone()))
                .unwrap_or_else(|| "medium".into()),
            status,
            category: input
                .category
                .or_else(|| existing.as_ref().map(|t| t.category.clone()))
                .unwrap_or_else(|| "general".into()),
            notes: input
                .notes
                .or_else(|| existing.as_ref().and_then(|t| t.notes.clone())),
            recurrence: input
                .recurrence
                .or_else(|| existing.as_ref().map(|t| t.recurrence.clone()))
                .unwrap_or_else(|| "none".into()),
            completed_at,
            created_at,
            updated_at: now,
        };
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO todos (id, title, description, deadline, priority, status, category, notes, recurrence, completed_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                 ON CONFLICT(id) DO UPDATE SET
                    title=excluded.title, description=excluded.description, deadline=excluded.deadline,
                    priority=excluded.priority, status=excluded.status, category=excluded.category,
                    notes=excluded.notes, recurrence=excluded.recurrence, completed_at=excluded.completed_at,
                    updated_at=excluded.updated_at",
                params![
                    todo.id,
                    todo.title,
                    todo.description,
                    todo.deadline,
                    todo.priority,
                    todo.status,
                    todo.category,
                    todo.notes,
                    todo.recurrence,
                    todo.completed_at,
                    todo.created_at,
                    todo.updated_at,
                ],
            )?;
            Ok(())
        })?;
        Ok(todo)
    }

    pub fn complete_todo(&self, id: &str) -> Result<Vec<Todo>, DbError> {
        let mut todo = self.get_todo(id)?;
        let today = local_today();
        todo.status = "completed".into();
        todo.completed_at = Some(chrono_now());
        todo.updated_at = chrono_now();
        let spawned = if todo.recurrence != "none" {
            todo.deadline
                .as_deref()
                .and_then(|d| next_deadline(d, &todo.recurrence))
                .map(|next| UpsertTodo {
                    title: todo.title.clone(),
                    description: todo.description.clone(),
                    deadline: Some(next),
                    priority: Some(todo.priority.clone()),
                    status: Some("not_started".into()),
                    category: Some(todo.category.clone()),
                    notes: todo.notes.clone(),
                    recurrence: Some(todo.recurrence.clone()),
                    ..Default::default()
                })
        } else {
            None
        };
        let saved = self.upsert_todo(UpsertTodo {
            id: Some(todo.id),
            title: todo.title,
            description: todo.description,
            deadline: todo.deadline,
            priority: Some(todo.priority),
            status: Some("completed".into()),
            category: Some(todo.category),
            notes: todo.notes,
            recurrence: Some(todo.recurrence),
        })?;
        let mut out = vec![saved];
        if let Some(next) = spawned {
            out.push(self.upsert_todo(next)?);
        }
        let _ = today;
        Ok(out)
    }

    pub fn delete_todo(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM todos WHERE id = ?1", params![id])?;
            if n == 0 {
                return Err(DbError::NotFound(id.to_string()));
            }
            Ok(())
        })
    }

    pub fn format_open_todos_context(&self) -> Option<String> {
        let todos = self.list_todos(None, None).ok()?;
        let today = local_today();
        let open: Vec<_> = todos
            .into_iter()
            .filter(|t| t.status != "completed")
            .take(12)
            .collect();
        if open.is_empty() {
            return None;
        }
        let overdue = open.iter().filter(|t| todo_is_overdue(t, &today)).count();
        let lines: Vec<String> = open
            .iter()
            .map(|t| {
                let due = t.deadline.clone().unwrap_or_else(|| "—".into());
                format!(
                    "- [{}] {} (due {}, {})",
                    t.priority, t.title, due, t.status
                )
            })
            .collect();
        Some(format!("Open todos ({overdue} overdue):\n{}", lines.join("\n")))
    }
}

pub fn local_today() -> String {
    let secs = chrono_now() / 1000;
    // Approximate local date via UTC; UI stores YYYY-MM-DD from the client.
    let days = secs / 86400;
    ordinal_to_ymd(days as i32 + 1)
        .map(|(y, m, d)| format!("{y:04}-{m:02}-{d:02}"))
        .unwrap_or_else(|| "1970-01-01".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overdue_when_deadline_passed() {
        let todo = Todo {
            id: "1".into(),
            title: "x".into(),
            description: None,
            deadline: Some("2026-08-01".into()),
            priority: "high".into(),
            status: "in_progress".into(),
            category: "general".into(),
            notes: None,
            recurrence: "none".into(),
            completed_at: None,
            created_at: 0,
            updated_at: 0,
        };
        assert!(todo_is_overdue(&todo, "2026-08-08"));
        let mut done = todo.clone();
        done.status = "completed".into();
        assert!(!todo_is_overdue(&done, "2026-08-08"));
    }

    #[test]
    fn recurrence_shifts_deadline() {
        assert_eq!(
            next_deadline("2026-08-08", "daily").as_deref(),
            Some("2026-08-09")
        );
        assert_eq!(
            next_deadline("2026-08-08", "weekly").as_deref(),
            Some("2026-08-15")
        );
        assert_eq!(
            next_deadline("2026-08-08", "monthly").as_deref(),
            Some("2026-09-08")
        );
        assert_eq!(next_deadline("2026-08-08", "none"), None);
    }
}
