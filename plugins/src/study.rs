use std::sync::Arc;

use buddy_core::{
    parse_tool_json, AfterExecute, AskKind, BuddyPlugin, FieldSpec, Tool, ToolDecl, ToolError,
    ToolResult, ToolSchema,
};
use buddy_database::{
    local_today, study_deadline_risk, Database, StudyAssignment, StudySession, StudyTopic,
};
use serde::Deserialize;

pub struct StudyPlugin;

struct StudyStatusTool {
    db: Arc<Database>,
}
struct StudyLookTool {
    db: Arc<Database>,
}
struct StudyLogTool {
    db: Arc<Database>,
}
struct UpsertSubjectTool {
    db: Arc<Database>,
}
struct UpsertTopicTool {
    db: Arc<Database>,
}
struct UpsertAssignmentTool {
    db: Arc<Database>,
}

impl BuddyPlugin for StudyPlugin {
    fn id(&self) -> &'static str {
        "study"
    }

    fn tools(&self, db: Arc<Database>) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(StudyStatusTool { db: db.clone() }),
            Arc::new(StudyLookTool { db: db.clone() }),
            Arc::new(StudyLogTool { db: db.clone() }),
            Arc::new(UpsertSubjectTool { db: db.clone() }),
            Arc::new(UpsertTopicTool { db: db.clone() }),
            Arc::new(UpsertAssignmentTool { db }),
        ]
    }

    fn tool_decls(&self) -> &'static [ToolDecl] {
        &[
            ToolDecl {
                name: "study.status",
                planner_line: "study.status: progress, deadline risk, subjects, recent sessions. tool_input JSON: {\"subject_id?\":\"...\"}",
            },
            ToolDecl {
                name: "study.look",
                planner_line: "study.look: read study rows. tool_input JSON: {\"what\":\"sessions|subjects|topics|assignments|status\", \"subject_id?\":\"...\", \"limit?\":20}. Use when they ask what they studied. Look before upserting.",
            },
            ToolDecl {
                name: "study.log_session",
                planner_line: "study.log_session: log a study session. tool_input JSON: {\"topic_id?\":\"\", \"subject_id?\":\"\", \"date?\":\"YYYY-MM-DD\", \"duration_minutes\":60, \"notes?\":\"\"}",
            },
            ToolDecl {
                name: "study.upsert_subject",
                planner_line: "study.upsert_subject: create or rename a subject. tool_input JSON: {\"name\":\"English\", \"id?\":\"\", \"color?\":\"\"}",
            },
            ToolDecl {
                name: "study.upsert_topic",
                planner_line: "study.upsert_topic: create or update a topic (course unit). tool_input JSON: {\"name\":\"Intro to Cybersecurity\", \"subject_id?\":\"\", \"subject?\":\"Cybersecurity\", \"status?\":\"not_started\", \"deadline?\":\"YYYY-MM-DD\", \"priority?\":\"medium\", \"notes?\":\"\"}. For a pasted syllabus, one topic = the course title — not each 'Topics:' bullet.",
            },
            ToolDecl {
                name: "study.upsert_assignment",
                planner_line: "study.upsert_assignment: create or update graded work. tool_input JSON: {\"title\":\"Module 1 Quiz\", \"subject_id?\":\"\", \"subject?\":\"Cybersecurity\", \"topic_id?\":\"\", \"kind?\":\"assignment|exam\", \"status?\":\"not_started\", \"deadline?\":\"YYYY-MM-DD\", \"priority?\":\"medium\", \"notes?\":\"\"}. Module N + Assignment: Module Quiz → kind=assignment title Module N Quiz. Final Exam → kind=exam. Do not invent short fragment titles.",
            },
        ]
    }

    fn tool_schemas(&self) -> &'static [ToolSchema] {
        &[
            ToolSchema {
                tool: "study.log_session",
                fields: &[FieldSpec {
                    name: "duration_minutes",
                    label: "minutes studied",
                    required: true,
                    memory_keys: &[],
                    ask_kind: AskKind::Text,
                    choices: &[],
                }],
            },
            ToolSchema {
                tool: "study.upsert_topic",
                fields: &[FieldSpec {
                    name: "name",
                    label: "topic",
                    required: true,
                    memory_keys: &[],
                    ask_kind: AskKind::Text,
                    choices: &[],
                }],
            },
            ToolSchema {
                tool: "study.upsert_assignment",
                fields: &[FieldSpec {
                    name: "title",
                    label: "assignment",
                    required: true,
                    memory_keys: &[],
                    ask_kind: AskKind::Text,
                    choices: &[],
                }],
            },
        ]
    }

    fn after_execute_hint(&self, tool_name: &str) -> AfterExecute {
        if tool_name.starts_with("study.") && tool_name != "study.look" && tool_name != "study.status"
        {
            AfterExecute::EmitStudyUpdated
        } else {
            AfterExecute::None
        }
    }
}

fn resolve_subject_id(
    db: &Database,
    subject_id: Option<&str>,
    subject: Option<&str>,
) -> Result<String, ToolError> {
    if let Some(id) = subject_id.map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(id.to_string());
    }
    let subjects = db
        .list_study_subjects()
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
    if let Some(name) = subject.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(s) = subjects
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
        {
            return Ok(s.id.clone());
        }
        let created = db
            .upsert_study_subject(None, name, None)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        return Ok(created.id);
    }
    if let Some(s) = subjects.first() {
        return Ok(s.id.clone());
    }
    let created = db
        .upsert_study_subject(None, "General", None)
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
    Ok(created.id)
}

#[derive(Deserialize, Default)]
struct StatusIn {
    #[serde(default)]
    subject_id: Option<String>,
}

impl Tool for StudyStatusTool {
    fn name(&self) -> &str {
        "study.status"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: StatusIn = parse_tool_json(input, "study.status").unwrap_or_default();
        let today = local_today();
        let pace = self.db.recent_study_minutes_per_day(14).unwrap_or(0.0);
        let planned = self.db.planned_study_minutes_ahead(14).unwrap_or(0.0);
        let topics = self
            .db
            .list_study_topics(parsed.subject_id.as_deref())
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let assignments = self
            .db
            .list_study_assignments(parsed.subject_id.as_deref())
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let topic_rows: Vec<_> = topics
            .iter()
            .map(|t| {
                let risk = study_deadline_risk(
                    &t.status,
                    t.deadline.as_deref(),
                    t.remaining_estimate,
                    pace,
                    planned,
                    &t.priority,
                    &today,
                );
                serde_json::json!({
                    "id": t.id,
                    "name": t.name,
                    "status": t.status,
                    "deadline": t.deadline,
                    "last_studied": t.last_studied,
                    "risk": risk.status,
                    "reason": risk.reason,
                })
            })
            .collect();
        let assign_rows: Vec<_> = assignments
            .iter()
            .map(|a| {
                let risk = study_deadline_risk(
                    &a.status,
                    a.deadline.as_deref(),
                    None,
                    pace,
                    planned,
                    &a.priority,
                    &today,
                );
                serde_json::json!({
                    "id": a.id,
                    "title": a.title,
                    "kind": a.kind,
                    "status": a.status,
                    "deadline": a.deadline,
                    "risk": risk.status,
                    "reason": risk.reason,
                })
            })
            .collect();
        let subjects = self.db.list_study_subjects().unwrap_or_default();
        let sessions = self.db.list_study_sessions(12).unwrap_or_default();
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&serde_json::json!({
                "minutes_per_day": pace,
                "subjects": subjects,
                "topics": topic_rows,
                "assignments": assign_rows,
                "recent_sessions": sessions,
            }))
            .unwrap_or_default(),
        })
    }
}

#[derive(Deserialize, Default)]
struct LookIn {
    #[serde(default)]
    what: Option<String>,
    #[serde(default)]
    subject_id: Option<String>,
    #[serde(default)]
    limit: Option<i64>,
}

impl Tool for StudyLookTool {
    fn name(&self) -> &str {
        "study.look"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: LookIn = parse_tool_json(input, "study.look").unwrap_or_default();
        let what = parsed
            .what
            .as_deref()
            .unwrap_or("status")
            .trim()
            .to_ascii_lowercase();
        let subject = parsed.subject_id.as_deref().filter(|s| !s.is_empty());
        let limit = parsed.limit.unwrap_or(20).clamp(1, 80);
        let output = match what.as_str() {
            "sessions" => serde_json::to_string_pretty(
                &self
                    .db
                    .list_study_sessions(limit)
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?,
            ),
            "subjects" => serde_json::to_string_pretty(
                &self
                    .db
                    .list_study_subjects()
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?,
            ),
            "topics" => serde_json::to_string_pretty(
                &self
                    .db
                    .list_study_topics(subject)
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?,
            ),
            "assignments" => serde_json::to_string_pretty(
                &self
                    .db
                    .list_study_assignments(subject)
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?,
            ),
            _ => {
                return StudyStatusTool {
                    db: self.db.clone(),
                }
                .execute(&serde_json::json!({ "subject_id": subject }).to_string())
            }
        }
        .unwrap_or_default();
        Ok(ToolResult { output })
    }
}

#[derive(Deserialize)]
struct LogIn {
    #[serde(default)]
    topic_id: Option<String>,
    #[serde(default)]
    subject_id: Option<String>,
    #[serde(default)]
    date: Option<String>,
    duration_minutes: i64,
    #[serde(default)]
    notes: Option<String>,
}

impl Tool for StudyLogTool {
    fn name(&self) -> &str {
        "study.log_session"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: LogIn = parse_tool_json(input, "study.log_session")?;
        let session = self
            .db
            .log_study_session(StudySession {
                id: String::new(),
                subject_id: parsed.subject_id,
                topic_id: parsed.topic_id,
                date: parsed.date.unwrap_or_default(),
                duration_minutes: parsed.duration_minutes,
                notes: parsed.notes,
                created_at: 0,
            })
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&session).unwrap_or_default(),
        })
    }
}

#[derive(Deserialize)]
struct SubjectIn {
    name: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    color: Option<String>,
}

impl Tool for UpsertSubjectTool {
    fn name(&self) -> &str {
        "study.upsert_subject"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let p: SubjectIn = parse_tool_json(input, "study.upsert_subject")?;
        let name = p.name.trim();
        if name.is_empty() {
            return Err(ToolError::ExecutionFailed("subject name required".into()));
        }
        let s = self
            .db
            .upsert_study_subject(p.id.filter(|s| !s.trim().is_empty()), name, p.color)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&s).unwrap_or_default(),
        })
    }
}

#[derive(Deserialize)]
struct TopicIn {
    name: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    subject_id: Option<String>,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    deadline: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    remaining_estimate: Option<f64>,
    #[serde(default)]
    notes: Option<String>,
}

impl Tool for UpsertTopicTool {
    fn name(&self) -> &str {
        "study.upsert_topic"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let p: TopicIn = parse_tool_json(input, "study.upsert_topic")?;
        let name = p.name.trim();
        if name.is_empty() {
            return Err(ToolError::ExecutionFailed("topic name required".into()));
        }
        let subject_id = resolve_subject_id(
            &self.db,
            p.subject_id.as_deref(),
            p.subject.as_deref(),
        )?;
        let topic = self
            .db
            .upsert_study_topic(StudyTopic {
                id: p.id.unwrap_or_default(),
                subject_id,
                name: name.to_string(),
                status: p
                    .status
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "not_started".into()),
                last_studied: None,
                deadline: p.deadline.filter(|s| !s.is_empty()),
                priority: p
                    .priority
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "medium".into()),
                remaining_estimate: p.remaining_estimate,
                notes: p.notes.filter(|s| !s.is_empty()),
                created_at: 0,
                updated_at: 0,
            })
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&topic).unwrap_or_default(),
        })
    }
}

#[derive(Deserialize)]
struct AssignmentIn {
    title: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    subject_id: Option<String>,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    topic_id: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    deadline: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

impl Tool for UpsertAssignmentTool {
    fn name(&self) -> &str {
        "study.upsert_assignment"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let p: AssignmentIn = parse_tool_json(input, "study.upsert_assignment")?;
        let title = p.title.trim();
        if title.is_empty() {
            return Err(ToolError::ExecutionFailed("assignment title required".into()));
        }
        let subject_id = resolve_subject_id(
            &self.db,
            p.subject_id.as_deref(),
            p.subject.as_deref(),
        )?;
        let a = self
            .db
            .upsert_study_assignment(StudyAssignment {
                id: p.id.unwrap_or_default(),
                subject_id,
                topic_id: p.topic_id.filter(|s| !s.is_empty()),
                title: title.to_string(),
                kind: p
                    .kind
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "assignment".into()),
                status: p
                    .status
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "not_started".into()),
                deadline: p.deadline.filter(|s| !s.is_empty()),
                priority: p
                    .priority
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "medium".into()),
                notes: p.notes.filter(|s| !s.is_empty()),
                created_at: 0,
                updated_at: 0,
            })
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&a).unwrap_or_default(),
        })
    }
}
