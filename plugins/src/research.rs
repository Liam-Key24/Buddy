use std::sync::Arc;

use buddy_core::{
    parse_tool_json, AfterExecute, AskKind, BuddyPlugin, FieldSpec, Tool, ToolDecl, ToolError,
    ToolResult, ToolSchema, ToolSpec,
};
use buddy_database::{Database, UpdateResearchInput};
use serde::Deserialize;

pub struct ResearchPlugin;

struct ResearchUpdateTool {
    db: Arc<Database>,
}
struct ResearchListTool {
    db: Arc<Database>,
}
struct ResearchGetTool {
    db: Arc<Database>,
}

const RESEARCH_SPECS: &[ToolSpec] = &[
            ToolSpec::basic("research.list", "list saved research sessions", r#"research.list"#, buddy_core::empty_schema("research.list")),
            ToolSpec::basic("research.get", "read one research session", r#"research.get"#, buddy_core::empty_schema("research.get")),
            ToolSpec::basic("research.update", "save structured research findings", r#"research.update conversation_id=<id> summary="...""#, buddy_core::empty_schema("research.update")),
        ];

impl BuddyPlugin for ResearchPlugin {
    fn id(&self) -> &'static str {
        "research"
    }

    fn tools(&self, db: Arc<Database>) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(ResearchListTool { db: db.clone() }),
            Arc::new(ResearchGetTool { db: db.clone() }),
            Arc::new(ResearchUpdateTool { db }),
        ]
    }

    fn tool_decls(&self) -> &'static [ToolDecl] {
        &[
            ToolDecl {
                name: "research.list",
                planner_line: "research.list: list saved research sessions (titles, questions, summaries). tool_input JSON: {}",
            },
            ToolDecl {
                name: "research.get",
                planner_line: "research.get: read one research session. tool_input JSON: {\"conversation_id?\":\"...\", \"id?\":\"...\", \"query?\":\"title or question text\"}. conversation_id is filled from the current chat if omitted.",
            },
            ToolDecl {
                name: "research.update",
                planner_line: "research.update: save structured research findings for the current research conversation. tool_input JSON: {\"conversation_id\":\"...\", \"title?\":\"\", \"question?\":\"\", \"summary?\":\"\", \"findings?\":[\"...\"], \"sources?\":[\"...\"], \"details?\":\"\", \"open_questions?\":\"\", \"next_steps?\":\"\", \"notes?\":\"\"}",
            },
        ]
    }

    fn tool_specs(&self) -> &'static [ToolSpec] {
        RESEARCH_SPECS
    }

    fn tool_schemas(&self) -> &'static [ToolSchema] {
        &[ToolSchema {
            tool: "research.update",
            fields: &[FieldSpec {
                name: "conversation_id",
                label: "conversation id",
                required: true,
                memory_keys: &[],
                ask_kind: AskKind::Text,
                choices: &[],
            }],
        }]
    }

    fn after_execute_hint(&self, tool_name: &str) -> AfterExecute {
        if tool_name == "research.update" {
            AfterExecute::EmitResearchUpdated
        } else {
            AfterExecute::None
        }
    }
}

impl Tool for ResearchListTool {
    fn name(&self) -> &str {
        "research.list"
    }
    fn execute(&self, _input: &str) -> Result<ToolResult, ToolError> {
        let rows: Vec<_> = self
            .db
            .list_research_sessions()
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            .into_iter()
            .map(|s| {
                let summary = if s.summary.chars().count() > 240 {
                    format!("{}…", s.summary.chars().take(240).collect::<String>())
                } else {
                    s.summary.clone()
                };
                serde_json::json!({
                    "id": s.id,
                    "conversation_id": s.conversation_id,
                    "title": s.title,
                    "question": s.question,
                    "summary": summary,
                    "findings_count": s.findings.len(),
                    "updated_at": s.updated_at,
                })
            })
            .collect();
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&rows).unwrap_or_default(),
        })
    }
}

#[derive(Deserialize, Default)]
struct GetIn {
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    query: Option<String>,
}

impl Tool for ResearchGetTool {
    fn name(&self) -> &str {
        "research.get"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: GetIn = parse_tool_json(input, "research.get").unwrap_or_default();
        let sessions = self
            .db
            .list_research_sessions()
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        if let Some(cid) = parsed.conversation_id.as_deref().filter(|s| !s.is_empty()) {
            if let Some(s) = sessions.iter().find(|s| s.conversation_id == cid) {
                return Ok(ToolResult {
                    output: serde_json::to_string_pretty(s).unwrap_or_default(),
                });
            }
        }
        if let Some(id) = parsed.id.as_deref().filter(|s| !s.is_empty()) {
            if let Some(s) = sessions.iter().find(|s| s.id == id) {
                return Ok(ToolResult {
                    output: serde_json::to_string_pretty(s).unwrap_or_default(),
                });
            }
        }
        if let Some(q) = parsed.query.as_deref().filter(|s| !s.is_empty()) {
            let q = q.to_ascii_lowercase();
            if let Some(s) = sessions.iter().find(|s| {
                s.title.to_ascii_lowercase().contains(&q)
                    || s.question.to_ascii_lowercase().contains(&q)
            }) {
                return Ok(ToolResult {
                    output: serde_json::to_string_pretty(s).unwrap_or_default(),
                });
            }
        }
        if sessions.len() == 1 {
            return Ok(ToolResult {
                output: serde_json::to_string_pretty(&sessions[0]).unwrap_or_default(),
            });
        }
        Err(ToolError::ExecutionFailed(
            "no matching research session — call research.list".into(),
        ))
    }
}

#[derive(Deserialize)]
struct UpdateIn {
    conversation_id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    question: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    findings: Option<Vec<String>>,
    #[serde(default)]
    sources: Option<Vec<String>>,
    #[serde(default)]
    details: Option<String>,
    #[serde(default)]
    open_questions: Option<String>,
    #[serde(default)]
    next_steps: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

impl Tool for ResearchUpdateTool {
    fn name(&self) -> &str {
        "research.update"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: UpdateIn = parse_tool_json(input, "research.update")?;
        let _ = self
            .db
            .ensure_research_session(&parsed.conversation_id, parsed.title.as_deref().unwrap_or(""), parsed.question.as_deref().unwrap_or(""));
        let session = self
            .db
            .update_research_session(
                &parsed.conversation_id,
                UpdateResearchInput {
                    title: parsed.title,
                    question: parsed.question,
                    summary: parsed.summary,
                    findings: parsed.findings,
                    sources: parsed.sources,
                    details: parsed.details,
                    open_questions: parsed.open_questions,
                    next_steps: parsed.next_steps,
                    notes: parsed.notes,
                },
            )
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&session).unwrap_or_default(),
        })
    }
}
