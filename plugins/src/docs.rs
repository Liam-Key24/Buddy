use std::sync::Arc;

use buddy_core::{
    parse_tool_json, AfterExecute, AskKind, BuddyPlugin, FieldSpec, Tool, ToolDecl, ToolError,
    ToolResult, ToolSchema, ToolSpec,
};
use buddy_database::{prepare_document_content, Database};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

pub struct DocsPlugin;

struct DocsSearchTool {
    db: Arc<Database>,
}
struct DocsGetTool {
    db: Arc<Database>,
}
struct DocsListTool {
    db: Arc<Database>,
}
struct DocsUpsertTool {
    db: Arc<Database>,
}
struct DocsFormatTool {
    db: Arc<Database>,
}
struct DocsPatchTool {
    db: Arc<Database>,
}
struct DocsDeleteTool {
    db: Arc<Database>,
}

const DOCS_SPECS: &[ToolSpec] = &[
            ToolSpec::basic("docs.search", "search saved Documents", r#"docs.search query="notes""#, buddy_core::empty_schema("docs.search")),
            ToolSpec::basic("docs.get", "read one document by id or title", r#"docs.get id=bello.today"#, buddy_core::empty_schema("docs.get")),
            ToolSpec::basic("docs.list", "list document titles", r#"docs.list"#, buddy_core::empty_schema("docs.list")),
            ToolSpec::basic("docs.format", "improve layout of an existing Document", r#"docs.format id=bello.today"#, buddy_core::empty_schema("docs.format")),
            ToolSpec::basic("docs.patch", "replace one exact snippet", r#"docs.patch id=bello.today find=old replace=new"#, buddy_core::empty_schema("docs.patch")),
            ToolSpec::basic("docs.upsert", "create or replace an in-app Document", r#"docs.upsert title=notes content="...""#, buddy_core::empty_schema("docs.upsert")),
            ToolSpec::basic("docs.delete", "delete an in-app Document", r#"docs.delete id=bello.today"#, buddy_core::empty_schema("docs.delete")),
        ];

impl BuddyPlugin for DocsPlugin {
    fn id(&self) -> &'static str {
        "docs"
    }

    fn tools(&self, db: Arc<Database>) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(DocsSearchTool { db: db.clone() }),
            Arc::new(DocsGetTool { db: db.clone() }),
            Arc::new(DocsListTool { db: db.clone() }),
            Arc::new(DocsFormatTool { db: db.clone() }),
            Arc::new(DocsPatchTool { db: db.clone() }),
            Arc::new(DocsUpsertTool { db: db.clone() }),
            Arc::new(DocsDeleteTool { db }),
        ]
    }

    fn tool_decls(&self) -> &'static [ToolDecl] {
        &[
            ToolDecl {
                name: "docs.search",
                planner_line: "docs.search: search saved Documents. tool_input JSON: {\"query\":\"...\", \"limit\":5}. Returns snippets — follow with docs.get for full text.",
            },
            ToolDecl {
                name: "docs.get",
                planner_line: "docs.get: read one document by id or title. tool_input JSON: {\"id\":\"bello.today\"}",
            },
            ToolDecl {
                name: "docs.list",
                planner_line: "docs.list: list document titles. tool_input JSON: {\"folder_id?\":\"...\"}",
            },
            ToolDecl {
                name: "docs.format",
                planner_line: "docs.format: improve layout of an existing in-app Document (headings, lists, spacing). Tiny call — do NOT rewrite the body. tool_input JSON: {\"id\":\"bello.today\"}. Use for better/cleaner format.",
            },
            ToolDecl {
                name: "docs.patch",
                planner_line: "docs.patch: replace one exact snippet in an existing Document. tool_input JSON: {\"id\":\"bello.today\",\"find\":\"old\",\"replace\":\"new\"}. Prefer this over docs.upsert for small edits.",
            },
            ToolDecl {
                name: "docs.upsert",
                planner_line: "docs.upsert: create or replace an in-app Document (not a disk file). tool_input JSON: {\"title\":\"bello.today\", \"content\":\"markdown body\", \"id?\":\"optional id or existing title\", \"format?\":\"markdown|html|csv\"}. Pass the user's paste through with only light cleanup — do not fully rewrite long markdown. For better format use docs.format. write_file is home-folder disk only.",
            },
            ToolDecl {
                name: "docs.delete",
                planner_line: "docs.delete: delete an in-app Document by id or title. tool_input JSON: {\"id\":\"bello.today\"}",
            },
        ]
    }

    fn tool_specs(&self) -> &'static [ToolSpec] {
        DOCS_SPECS
    }

    fn tool_schemas(&self) -> &'static [ToolSchema] {
        &[
            ToolSchema {
                tool: "docs.search",
                fields: &[FieldSpec {
                    name: "query",
                    label: "search query",
                    required: true,
                    memory_keys: &[],
                    ask_kind: AskKind::Text,
                    choices: &[],
                }],
            },
            ToolSchema {
                tool: "docs.get",
                fields: &[FieldSpec {
                    name: "id",
                    label: "document id or title",
                    required: true,
                    memory_keys: &[],
                    ask_kind: AskKind::Text,
                    choices: &[],
                }],
            },
            ToolSchema {
                tool: "docs.format",
                fields: &[FieldSpec {
                    name: "id",
                    label: "document id or title",
                    required: true,
                    memory_keys: &[],
                    ask_kind: AskKind::Text,
                    choices: &[],
                }],
            },
            ToolSchema {
                tool: "docs.patch",
                fields: &[
                    FieldSpec {
                        name: "id",
                        label: "document id or title",
                        required: true,
                        memory_keys: &[],
                        ask_kind: AskKind::Text,
                        choices: &[],
                    },
                    FieldSpec {
                        name: "find",
                        label: "exact text to replace",
                        required: true,
                        memory_keys: &[],
                        ask_kind: AskKind::Text,
                        choices: &[],
                    },
                    FieldSpec {
                        name: "replace",
                        label: "replacement text",
                        required: true,
                        memory_keys: &[],
                        ask_kind: AskKind::Text,
                        choices: &[],
                    },
                ],
            },
            ToolSchema {
                tool: "docs.upsert",
                fields: &[
                    FieldSpec {
                        name: "title",
                        label: "document title",
                        required: true,
                        memory_keys: &[],
                        ask_kind: AskKind::Text,
                        choices: &[],
                    },
                    FieldSpec {
                        name: "content",
                        label: "document body",
                        required: false,
                        memory_keys: &[],
                        ask_kind: AskKind::Text,
                        choices: &[],
                    },
                ],
            },
            ToolSchema {
                tool: "docs.delete",
                fields: &[FieldSpec {
                    name: "id",
                    label: "document id or title",
                    required: true,
                    memory_keys: &[],
                    ask_kind: AskKind::Text,
                    choices: &[],
                }],
            },
        ]
    }

    fn after_execute_hint(&self, tool_name: &str) -> AfterExecute {
        match tool_name {
            "docs.upsert" | "docs.format" | "docs.patch" | "docs.delete" => {
                AfterExecute::EmitDocsUpdated
            }
            _ => AfterExecute::None,
        }
    }
}

#[derive(Deserialize)]
struct SearchIn {
    query: String,
    #[serde(default)]
    limit: Option<i64>,
}

impl Tool for DocsSearchTool {
    fn name(&self) -> &str {
        "docs.search"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: SearchIn = parse_tool_json(input, "docs.search")?;
        let hits = self
            .db
            .search_documents(&parsed.query, parsed.limit.unwrap_or(8))
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&hits).unwrap_or_default(),
        })
    }
}

#[derive(Deserialize)]
struct IdIn {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

fn id_or_title(parsed: &IdIn) -> Result<String, ToolError> {
    parsed
        .id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            parsed
                .title
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })
        .map(|s| s.to_string())
        .ok_or_else(|| ToolError::ExecutionFailed("document id or title required".into()))
}

impl Tool for DocsGetTool {
    fn name(&self) -> &str {
        "docs.get"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: IdIn = parse_tool_json(input, "docs.get")?;
        let key = id_or_title(&parsed)?;
        let doc = self
            .db
            .get_document(&key)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&doc).unwrap_or_default(),
        })
    }
}

#[derive(Deserialize, Default)]
struct ListIn {
    #[serde(default)]
    folder_id: Option<String>,
}

impl Tool for DocsListTool {
    fn name(&self) -> &str {
        "docs.list"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: ListIn = parse_tool_json(input, "docs.list").unwrap_or_default();
        let docs = self
            .db
            .list_documents(parsed.folder_id.as_deref())
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let slim: Vec<_> = docs
            .into_iter()
            .map(|d| {
                json!({"id": d.id, "title": d.title, "format": d.format, "pinned": d.pinned, "updated_at": d.updated_at})
            })
            .collect();
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&slim).unwrap_or_default(),
        })
    }
}

#[derive(Deserialize)]
struct UpsertIn {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    folder_id: Option<String>,
    #[serde(default)]
    pinned: Option<bool>,
}

impl Tool for DocsUpsertTool {
    fn name(&self) -> &str {
        "docs.upsert"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: UpsertIn = parse_tool_json(input, "docs.upsert")?;
        // #endregion
        let id_raw = parsed
            .id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let title_raw = parsed
            .title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let existing = id_raw
            .and_then(|id| self.db.get_document(id).ok())
            .or_else(|| title_raw.and_then(|t| self.db.get_document(t).ok()));

        let title = title_raw
            .map(|s| s.to_string())
            .or_else(|| existing.as_ref().map(|d| d.title.clone()))
            .or_else(|| {
                id_raw
                    .filter(|id| Uuid::parse_str(id).is_err())
                    .map(|s| s.to_string())
            })
            .ok_or_else(|| ToolError::ExecutionFailed("docs.upsert needs a title".into()))?;

        let (format, content) = if let Some(body) = parsed.content.as_deref() {
            if body.trim().is_empty() {
                if let Some(d) = existing.as_ref() {
                    let prev = d.content.trim();
                    if !prev.is_empty() && prev != "<p></p>" {
                        return Err(ToolError::ExecutionFailed(
                            "docs.upsert refused an empty overwrite. Use docs.format to reformat, or docs.patch for a small edit.".into(),
                        ));
                    }
                }
            }
            prepare_document_content(parsed.format.as_deref().unwrap_or(""), body)
        } else if let Some(d) = existing.as_ref() {
            (d.format.clone(), d.content.clone())
        } else {
            ("html".into(), "<p></p>".into())
        };

        let id = existing.as_ref().map(|d| d.id.clone()).or_else(|| {
            id_raw
                .filter(|id| Uuid::parse_str(id).is_ok())
                .map(|s| s.to_string())
        });
        let created = existing.is_none();
        let folder_id = parsed
            .folder_id
            .or_else(|| existing.as_ref().and_then(|d| d.folder_id.clone()));

        let doc = self
            .db
            .upsert_document(
                id,
                folder_id,
                &title,
                &format,
                &content,
                parsed.pinned,
            )
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: json!({
                "ok": true,
                "action": if created { "created" } else { "updated" },
                "id": doc.id,
                "title": doc.title,
                "format": doc.format,
            })
            .to_string(),
        })
    }
}

#[derive(Deserialize)]
struct PatchIn {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    find: String,
    #[serde(default)]
    replace: Option<String>,
}

impl Tool for DocsFormatTool {
    fn name(&self) -> &str {
        "docs.format"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: IdIn = parse_tool_json(input, "docs.format")?;
        let key = id_or_title(&parsed)?;
        let doc = self
            .db
            .format_document(&key)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: json!({
                "ok": true,
                "action": "formatted",
                "id": doc.id,
                "title": doc.title,
                "format": doc.format,
            })
            .to_string(),
        })
    }
}

impl Tool for DocsPatchTool {
    fn name(&self) -> &str {
        "docs.patch"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: PatchIn = parse_tool_json(input, "docs.patch")?;
        let key = id_or_title(&IdIn {
            id: parsed.id.clone(),
            title: parsed.title.clone(),
        })?;
        let doc = self
            .db
            .patch_document(&key, &parsed.find, parsed.replace.as_deref().unwrap_or(""))
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: json!({
                "ok": true,
                "action": "patched",
                "id": doc.id,
                "title": doc.title,
                "format": doc.format,
            })
            .to_string(),
        })
    }
}

impl Tool for DocsDeleteTool {
    fn name(&self) -> &str {
        "docs.delete"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: IdIn = parse_tool_json(input, "docs.delete")?;
        let key = id_or_title(&parsed)?;
        let doc = self
            .db
            .get_document(&key)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        self.db
            .delete_document(&doc.id)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: json!({
                "ok": true,
                "action": "deleted",
                "id": doc.id,
                "title": doc.title,
            })
            .to_string(),
        })
    }
}
