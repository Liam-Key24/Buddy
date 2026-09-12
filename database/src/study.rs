use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{chrono_now, local_today, BuddyCalendarEventRow, Database, DbError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudySubject {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyTopic {
    pub id: String,
    pub subject_id: String,
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_studied: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    pub priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_estimate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyAssignment {
    pub id: String,
    pub subject_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    pub priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudySession {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_id: Option<String>,
    pub date: String,
    pub duration_minutes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyRisk {
    pub status: String,
    pub reason: String,
}

pub fn days_until(deadline: &str, today: &str) -> Option<i64> {
    let a = parse_ymd(deadline)?;
    let b = parse_ymd(today)?;
    Some((a - b) as i64)
}

fn parse_ymd(s: &str) -> Option<i32> {
    let p: Vec<i32> = s.split('-').filter_map(|x| x.parse().ok()).collect();
    if p.len() != 3 {
        return None;
    }
    crate::todos::ymd_to_ordinal_pub(p[0], p[1], p[2])
}

/// Estimate whether a topic/assignment will finish before its deadline.
pub fn study_deadline_risk(
    status: &str,
    deadline: Option<&str>,
    remaining_hours: Option<f64>,
    recent_minutes_per_day: f64,
    planned_minutes: f64,
    priority: &str,
    today: &str,
) -> StudyRisk {
    if status == "completed" {
        return StudyRisk {
            status: "on_track".into(),
            reason: "Already completed.".into(),
        };
    }
    let Some(deadline) = deadline.filter(|d| !d.is_empty()) else {
        return StudyRisk {
            status: "on_track".into(),
            reason: "No deadline set.".into(),
        };
    };
    let Some(days) = days_until(deadline, today) else {
        return StudyRisk {
            status: "at_risk".into(),
            reason: "Deadline date is invalid.".into(),
        };
    };
    if days < 0 {
        return StudyRisk {
            status: "unlikely".into(),
            reason: format!("Deadline was {} days ago.", -days),
        };
    }
    if status == "not_started" && days <= 2 && priority != "low" {
        return StudyRisk {
            status: "at_risk".into(),
            reason: format!("Not started with {days} day(s) left."),
        };
    }
    let remaining = remaining_hours.unwrap_or(if status == "not_started" {
        4.0
    } else {
        2.0
    });
    let capacity_hours = (days as f64 + 1.0) * (recent_minutes_per_day + planned_minutes / (days as f64 + 1.0).max(1.0))
        / 60.0;
    if remaining > capacity_hours * 1.4 {
        return StudyRisk {
            status: "unlikely".into(),
            reason: format!(
                "About {remaining:.1}h left vs ~{capacity_hours:.1}h available before {deadline}."
            ),
        };
    }
    if remaining > capacity_hours || (days <= 3 && status != "in_progress") {
        return StudyRisk {
            status: "at_risk".into(),
            reason: format!(
                "{remaining:.1}h remaining with {days} day(s) left; pace may not be enough."
            ),
        };
    }
    StudyRisk {
        status: "on_track".into(),
        reason: format!("{days} day(s) left; current pace looks sufficient."),
    }
}

impl Database {
    pub fn list_study_subjects(&self) -> Result<Vec<StudySubject>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, color, created_at, updated_at FROM study_subjects ORDER BY name",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(StudySubject {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn upsert_study_subject(
        &self,
        id: Option<String>,
        name: &str,
        color: Option<String>,
    ) -> Result<StudySubject, DbError> {
        let now = chrono_now();
        let id = id.filter(|s| !s.is_empty()).unwrap_or_else(|| Uuid::new_v4().to_string());
        let created = self
            .list_study_subjects()?
            .into_iter()
            .find(|s| s.id == id)
            .map(|s| s.created_at)
            .unwrap_or(now);
        let subject = StudySubject {
            id: id.clone(),
            name: name.trim().to_string(),
            color,
            created_at: created,
            updated_at: now,
        };
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO study_subjects (id, name, color, created_at, updated_at) VALUES (?1,?2,?3,?4,?5)
                 ON CONFLICT(id) DO UPDATE SET name=excluded.name, color=excluded.color, updated_at=excluded.updated_at",
                params![subject.id, subject.name, subject.color, subject.created_at, subject.updated_at],
            )?;
            Ok(())
        })?;
        Ok(subject)
    }

    pub fn delete_study_subject(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM study_subjects WHERE id=?1", params![id])?;
            if n == 0 {
                Err(DbError::NotFound(id.into()))
            } else {
                Ok(())
            }
        })
    }

    pub fn list_study_topics(&self, subject_id: Option<&str>) -> Result<Vec<StudyTopic>, DbError> {
        self.with_conn(|conn| {
            let sql = if subject_id.is_some() {
                "SELECT id, subject_id, name, status, last_studied, deadline, priority, remaining_estimate, notes, created_at, updated_at FROM study_topics WHERE subject_id=?1 ORDER BY deadline IS NULL, deadline"
            } else {
                "SELECT id, subject_id, name, status, last_studied, deadline, priority, remaining_estimate, notes, created_at, updated_at FROM study_topics ORDER BY deadline IS NULL, deadline"
            };
            let mut stmt = conn.prepare(sql)?;
            let map = |row: &rusqlite::Row<'_>| {
                Ok(StudyTopic {
                    id: row.get(0)?,
                    subject_id: row.get(1)?,
                    name: row.get(2)?,
                    status: row.get(3)?,
                    last_studied: row.get(4)?,
                    deadline: row.get(5)?,
                    priority: row.get(6)?,
                    remaining_estimate: row.get(7)?,
                    notes: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            };
            let rows = if let Some(sid) = subject_id {
                stmt.query_map(params![sid], map)?
            } else {
                stmt.query_map([], map)?
            };
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn upsert_study_topic(&self, topic: StudyTopic) -> Result<StudyTopic, DbError> {
        let now = chrono_now();
        let mut t = topic;
        if t.id.is_empty() {
            t.id = Uuid::new_v4().to_string();
            t.created_at = now;
        }
        t.updated_at = now;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO study_topics (id, subject_id, name, status, last_studied, deadline, priority, remaining_estimate, notes, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
                 ON CONFLICT(id) DO UPDATE SET subject_id=excluded.subject_id, name=excluded.name, status=excluded.status,
                    last_studied=excluded.last_studied, deadline=excluded.deadline, priority=excluded.priority,
                    remaining_estimate=excluded.remaining_estimate, notes=excluded.notes, updated_at=excluded.updated_at",
                params![t.id, t.subject_id, t.name, t.status, t.last_studied, t.deadline, t.priority, t.remaining_estimate, t.notes, t.created_at, t.updated_at],
            )?;
            Ok(())
        })?;
        Ok(t)
    }

    pub fn delete_study_topic(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM study_topics WHERE id=?1", params![id])?;
            if n == 0 {
                Err(DbError::NotFound(id.into()))
            } else {
                Ok(())
            }
        })
    }

    pub fn list_study_assignments(
        &self,
        subject_id: Option<&str>,
    ) -> Result<Vec<StudyAssignment>, DbError> {
        self.with_conn(|conn| {
            let sql = if subject_id.is_some() {
                "SELECT id, subject_id, topic_id, title, kind, status, deadline, priority, notes, created_at, updated_at FROM study_assignments WHERE subject_id=?1 ORDER BY deadline IS NULL, deadline"
            } else {
                "SELECT id, subject_id, topic_id, title, kind, status, deadline, priority, notes, created_at, updated_at FROM study_assignments ORDER BY deadline IS NULL, deadline"
            };
            let mut stmt = conn.prepare(sql)?;
            let map = |row: &rusqlite::Row<'_>| {
                Ok(StudyAssignment {
                    id: row.get(0)?,
                    subject_id: row.get(1)?,
                    topic_id: row.get(2)?,
                    title: row.get(3)?,
                    kind: row.get(4)?,
                    status: row.get(5)?,
                    deadline: row.get(6)?,
                    priority: row.get(7)?,
                    notes: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            };
            let rows = if let Some(sid) = subject_id {
                stmt.query_map(params![sid], map)?
            } else {
                stmt.query_map([], map)?
            };
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn upsert_study_assignment(&self, a: StudyAssignment) -> Result<StudyAssignment, DbError> {
        let now = chrono_now();
        let mut a = a;
        if a.id.is_empty() {
            a.id = Uuid::new_v4().to_string();
            a.created_at = now;
        }
        a.updated_at = now;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO study_assignments (id, subject_id, topic_id, title, kind, status, deadline, priority, notes, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
                 ON CONFLICT(id) DO UPDATE SET subject_id=excluded.subject_id, topic_id=excluded.topic_id, title=excluded.title,
                    kind=excluded.kind, status=excluded.status, deadline=excluded.deadline, priority=excluded.priority,
                    notes=excluded.notes, updated_at=excluded.updated_at",
                params![a.id, a.subject_id, a.topic_id, a.title, a.kind, a.status, a.deadline, a.priority, a.notes, a.created_at, a.updated_at],
            )?;
            Ok(())
        })?;
        Ok(a)
    }

    pub fn delete_study_assignment(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM study_assignments WHERE id=?1", params![id])?;
            if n == 0 {
                Err(DbError::NotFound(id.into()))
            } else {
                Ok(())
            }
        })
    }

    pub fn list_study_sessions(&self, limit: i64) -> Result<Vec<StudySession>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, subject_id, topic_id, date, duration_minutes, notes, created_at FROM study_sessions ORDER BY date DESC, created_at DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok(StudySession {
                    id: row.get(0)?,
                    subject_id: row.get(1)?,
                    topic_id: row.get(2)?,
                    date: row.get(3)?,
                    duration_minutes: row.get(4)?,
                    notes: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn log_study_session(&self, session: StudySession) -> Result<StudySession, DbError> {
        let now = chrono_now();
        let mut s = session;
        if s.id.is_empty() {
            s.id = Uuid::new_v4().to_string();
        }
        s.created_at = now;
        if s.date.is_empty() {
            s.date = local_today();
        }
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO study_sessions (id, subject_id, topic_id, date, duration_minutes, notes, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![s.id, s.subject_id, s.topic_id, s.date, s.duration_minutes, s.notes, s.created_at],
            )?;
            if let Some(tid) = &s.topic_id {
                conn.execute(
                    "UPDATE study_topics SET last_studied=?1, updated_at=?2 WHERE id=?3",
                    params![s.date, now, tid],
                )?;
            }
            Ok(())
        })?;
        Ok(s)
    }

    pub fn recent_study_minutes_per_day(&self, days: i64) -> Result<f64, DbError> {
        let sessions = self.list_study_sessions(80)?;
        let today = local_today();
        let cutoff = crate::todos::add_days_to_date(&today, -days).unwrap_or(today.clone());
        let total: i64 = sessions
            .iter()
            .filter(|s| s.date.as_str() >= cutoff.as_str())
            .map(|s| s.duration_minutes)
            .sum();
        Ok(total as f64 / days.max(1) as f64)
    }

    pub fn planned_study_minutes_ahead(&self, days: i64) -> Result<f64, DbError> {
        let start = chrono_now();
        let end = start + days * 86_400_000;
        let events = self.list_buddy_calendar_events(start, end)?;
        Ok(events
            .iter()
            .filter(|e| is_study_calendar_event(e))
            .map(|e| {
                let span = (e.end_time.min(end) - e.start_time.max(start)).max(0) as f64;
                span / 60_000.0
            })
            .sum())
    }

    pub fn format_study_digest(&self) -> Option<String> {
        let topics = self.list_study_topics(None).ok()?;
        let assignments = self.list_study_assignments(None).ok()?;
        let today = local_today();
        let mut items: Vec<(String, String, Option<String>, String)> = Vec::new();
        for t in topics.into_iter().filter(|t| t.status != "completed") {
            items.push((t.name, t.status, t.deadline, t.priority));
        }
        for a in assignments.into_iter().filter(|a| a.status != "completed") {
            items.push((a.title, a.status, a.deadline, a.priority));
        }
        items.sort_by(|a, b| a.2.cmp(&b.2));
        if items.is_empty() {
            return None;
        }
        let lines: Vec<String> = items
            .into_iter()
            .take(6)
            .map(|(name, status, deadline, _)| {
                let due = deadline.unwrap_or_else(|| "—".into());
                format!("- {name} [{status}] due {due}")
            })
            .collect();
        Some(format!("Study deadlines (as of {today}):\n{}", lines.join("\n")))
    }
}

fn is_study_calendar_event(e: &BuddyCalendarEventRow) -> bool {
    let hay = format!(
        "{} {} {}",
        e.title,
        e.description.as_deref().unwrap_or(""),
        e.category
    )
    .to_ascii_lowercase();
    hay.contains("study") || hay.contains("revision") || hay.contains("exam")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn risk_completed_is_on_track() {
        let r = study_deadline_risk("completed", Some("2026-08-01"), Some(2.0), 30.0, 0.0, "high", "2026-08-08");
        assert_eq!(r.status, "on_track");
    }

    #[test]
    fn risk_past_deadline_unlikely() {
        let r = study_deadline_risk("in_progress", Some("2026-08-01"), Some(4.0), 20.0, 0.0, "high", "2026-08-08");
        assert_eq!(r.status, "unlikely");
    }

    #[test]
    fn risk_too_much_work_unlikely() {
        let r = study_deadline_risk("in_progress", Some("2026-08-10"), Some(40.0), 10.0, 0.0, "high", "2026-08-08");
        assert_eq!(r.status, "unlikely");
    }
}
