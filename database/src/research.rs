use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{chrono_now, Database, DbError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchSession {
    pub id: String,
    pub conversation_id: String,
    pub title: String,
    pub question: String,
    pub summary: String,
    pub findings: Vec<String>,
    pub sources: Vec<String>,
    pub details: String,
    pub open_questions: String,
    pub next_steps: String,
    pub notes: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateResearchInput {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub findings: Option<Vec<String>>,
    #[serde(default)]
    pub sources: Option<Vec<String>>,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub open_questions: Option<String>,
    #[serde(default)]
    pub next_steps: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

fn parse_json_vec(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

impl Database {
    fn map_research(row: &rusqlite::Row<'_>) -> Result<ResearchSession, rusqlite::Error> {
        let findings_json: String = row.get(5)?;
        let sources_json: String = row.get(6)?;
        Ok(ResearchSession {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            title: row.get(2)?,
            question: row.get(3)?,
            summary: row.get(4)?,
            findings: parse_json_vec(&findings_json),
            sources: parse_json_vec(&sources_json),
            details: row.get(7)?,
            open_questions: row.get(8)?,
            next_steps: row.get(9)?,
            notes: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        })
    }

    pub fn list_research_sessions(&self) -> Result<Vec<ResearchSession>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, conversation_id, title, question, summary, findings_json, sources_json, details, open_questions, next_steps, notes, created_at, updated_at
                 FROM research_sessions ORDER BY updated_at DESC LIMIT 200",
            )?;
            let rows = stmt.query_map([], Self::map_research)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn get_research_by_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Option<ResearchSession>, DbError> {
        self.with_conn(|conn| {
            match conn.query_row(
                "SELECT id, conversation_id, title, question, summary, findings_json, sources_json, details, open_questions, next_steps, notes, created_at, updated_at
                 FROM research_sessions WHERE conversation_id = ?1",
                params![conversation_id],
                Self::map_research,
            ) {
                Ok(s) => Ok(Some(s)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(DbError::from(e)),
            }
        })
    }

    pub fn ensure_research_session(
        &self,
        conversation_id: &str,
        title: &str,
        question: &str,
    ) -> Result<ResearchSession, DbError> {
        if let Some(existing) = self.get_research_by_conversation(conversation_id)? {
            return Ok(existing);
        }
        let now = chrono_now();
        let session = ResearchSession {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_string(),
            title: if title.trim().is_empty() {
                "Research".into()
            } else {
                title.trim().to_string()
            },
            question: question.to_string(),
            summary: String::new(),
            findings: vec![],
            sources: vec![],
            details: String::new(),
            open_questions: String::new(),
            next_steps: String::new(),
            notes: String::new(),
            created_at: now,
            updated_at: now,
        };
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO research_sessions (id, conversation_id, title, question, summary, findings_json, sources_json, details, open_questions, next_steps, notes, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    session.id,
                    session.conversation_id,
                    session.title,
                    session.question,
                    session.summary,
                    "[]",
                    "[]",
                    session.details,
                    session.open_questions,
                    session.next_steps,
                    session.notes,
                    session.created_at,
                    session.updated_at,
                ],
            )?;
            Ok(())
        })?;
        Ok(session)
    }

    pub fn update_research_session(
        &self,
        conversation_id: &str,
        input: UpdateResearchInput,
    ) -> Result<ResearchSession, DbError> {
        let mut session = self
            .get_research_by_conversation(conversation_id)?
            .ok_or_else(|| DbError::NotFound(conversation_id.to_string()))?;
        if let Some(v) = input.title {
            session.title = v;
        }
        if let Some(v) = input.question {
            session.question = v;
        }
        if let Some(v) = input.summary {
            session.summary = v;
        }
        if let Some(v) = input.findings {
            session.findings = v;
        }
        if let Some(v) = input.sources {
            session.sources = v;
        }
        if let Some(v) = input.details {
            session.details = v;
        }
        if let Some(v) = input.open_questions {
            session.open_questions = v;
        }
        if let Some(v) = input.next_steps {
            session.next_steps = v;
        }
        if let Some(v) = input.notes {
            session.notes = v;
        }
        session.updated_at = chrono_now();
        let findings_json = serde_json::to_string(&session.findings).unwrap_or_else(|_| "[]".into());
        let sources_json = serde_json::to_string(&session.sources).unwrap_or_else(|_| "[]".into());
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE research_sessions SET title=?1, question=?2, summary=?3, findings_json=?4, sources_json=?5,
                 details=?6, open_questions=?7, next_steps=?8, notes=?9, updated_at=?10
                 WHERE conversation_id=?11",
                params![
                    session.title,
                    session.question,
                    session.summary,
                    findings_json,
                    sources_json,
                    session.details,
                    session.open_questions,
                    session.next_steps,
                    session.notes,
                    session.updated_at,
                    conversation_id,
                ],
            )?;
            Ok(())
        })?;
        Ok(session)
    }

    pub fn format_research_digest(&self) -> Option<String> {
        let sessions = self.list_research_sessions().ok()?;
        if sessions.is_empty() {
            return None;
        }
        let lines: Vec<String> = sessions
            .into_iter()
            .take(5)
            .map(|s| {
                let q = if s.question.is_empty() {
                    s.title
                } else {
                    s.question
                };
                format!("- {q}")
            })
            .collect();
        Some(format!("Recent research:\n{}", lines.join("\n")))
    }
}
