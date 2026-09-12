use std::sync::Arc;

use buddy_core::{
    parse_tool_json, AfterExecute, AskKind, BuddyPlugin, FieldSpec, RespondMode, Safety, Tool,
    ToolDecl, ToolError, ToolResult, ToolSchema, ToolSpec,
};
use buddy_database::{local_today, todo_is_overdue, Database, UpsertTodo};
use serde::Deserialize;
use serde_json::json;

pub struct TodosPlugin;

const TODO_ADD_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "title",
        label: "task",
        required: true,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
    FieldSpec {
        name: "deadline",
        label: "deadline",
        required: false,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
    FieldSpec {
        name: "priority",
        label: "priority",
        required: false,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
    FieldSpec {
        name: "description",
        label: "description",
        required: false,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
    FieldSpec {
        name: "category",
        label: "category",
        required: false,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
    FieldSpec {
        name: "notes",
        label: "notes",
        required: false,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
    FieldSpec {
        name: "recurrence",
        label: "recurrence",
        required: false,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
];

const TODO_ADD_SCHEMA: ToolSchema = ToolSchema {
    tool: "todo.add",
    fields: TODO_ADD_FIELDS,
};

const TODO_UPDATE_FIELDS: &[FieldSpec] = &[FieldSpec {
    name: "id",
    label: "task id",
    required: true,
    memory_keys: &[],
    ask_kind: AskKind::Text,
    choices: &[],
}];

const TODO_UPDATE_SCHEMA: ToolSchema = ToolSchema {
    tool: "todo.update",
    fields: TODO_UPDATE_FIELDS,
};

const TODO_LIST_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "status",
        label: "status",
        required: false,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
    FieldSpec {
        name: "category",
        label: "category",
        required: false,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
];

const TODO_LIST_SCHEMA: ToolSchema = ToolSchema {
    tool: "todo.list",
    fields: TODO_LIST_FIELDS,
};

fn extract_todo_add(text: &str) -> Option<String> {
    let lower = text.trim().to_ascii_lowercase();
    let idx = if let Some(i) = lower.find("remind me to ") {
        i + 13
    } else if let Some(i) = lower.find("add a todo ") {
        i + 11
    } else if let Some(i) = lower.find("add todo ") {
        i + 9
    } else if let Some(i) = lower.find("add a task ") {
        i + 11
    } else if let Some(i) = lower.find("add task ") {
        i + 9
    } else if let Some(i) = lower.find("todo: ") {
        i + 6
    } else if let Some(i) = lower.find("to-do: ") {
        i + 7
    } else {
        return None;
    };
    let title = text
        .get(idx..)
        .unwrap_or("")
        .trim()
        .trim_end_matches(['.', '!', '?'])
        .trim()
        .trim_start_matches(':')
        .trim();
    if title.len() < 2 {
        return None;
    }
    Some(json!({ "title": title }).to_string())
}

const TODO_SPECS: &[ToolSpec] = &[
    ToolSpec {
        name: "todo.add",
        description: "create a task",
        example: r#"todo.add title="Buy milk" deadline=2026-08-18 priority=high"#,
        schema: TODO_ADD_SCHEMA,
        aliases: &["/todo"],
        rest_field: Some("title"),
        safety: Safety::Immediate,
        respond: RespondMode::Passthrough,
        likely: &["remind me to", "add a todo", "add a task", "todo:"],
        extract: Some(extract_todo_add),
        permission: buddy_core::Permission::None,
        openai_properties_json: "",
    },
    ToolSpec {
        name: "todo.list",
        description: "list tasks",
        example: r#"todo.list status=not_started"#,
        schema: TODO_LIST_SCHEMA,
        aliases: &[],
        rest_field: None,
        safety: Safety::Immediate,
        permission: buddy_core::Permission::None,
        respond: RespondMode::Passthrough,
        likely: &[],
        extract: None,
        openai_properties_json: "",
    },
    ToolSpec {
        name: "todo.update",
        description: "update or complete a task",
        example: r#"todo.update id="<id>" action=complete"#,
        schema: TODO_UPDATE_SCHEMA,
        aliases: &[],
        rest_field: None,
        safety: Safety::Immediate,
        permission: buddy_core::Permission::None,
        respond: RespondMode::Passthrough,
        likely: &[],
        extract: None,
        openai_properties_json: "",
    },
];

pub struct TodoListTool {
    db: Arc<Database>,
}
pub struct TodoAddTool {
    db: Arc<Database>,
}
pub struct TodoUpdateTool {
    db: Arc<Database>,
}

impl BuddyPlugin for TodosPlugin {
    fn id(&self) -> &'static str {
        "todos"
    }

    fn tools(&self, db: Arc<Database>) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(TodoListTool { db: db.clone() }),
            Arc::new(TodoAddTool { db: db.clone() }),
            Arc::new(TodoUpdateTool { db }),
        ]
    }

    fn tool_decls(&self) -> &'static [ToolDecl] {
        &[
            ToolDecl {
                name: "todo.list",
                planner_line: "todo.list: list tasks. tool_input JSON: {\"status\":\"not_started|in_progress|completed?\", \"category\":\"optional\"}",
            },
            ToolDecl {
                name: "todo.add",
                planner_line: "todo.add: create a task. tool_input JSON: {\"title\":\"...\", \"description?\":\"\", \"deadline?\":\"YYYY-MM-DD\", \"priority\":\"low|medium|high|critical\", \"category?\":\"\", \"notes?\":\"\", \"recurrence\":\"none|daily|weekly|monthly\"}",
            },
            ToolDecl {
                name: "todo.update",
                planner_line: "todo.update: update or complete a task. tool_input JSON: {\"id\":\"...\", \"action\":\"update|complete|delete\", \"title?\":\"\", \"status?\":\"\", \"deadline?\":\"\", \"priority?\":\"\"}",
            },
        ]
    }

    fn tool_schemas(&self) -> &'static [ToolSchema] {
        &[TODO_ADD_SCHEMA, TODO_UPDATE_SCHEMA, TODO_LIST_SCHEMA]
    }

    fn tool_specs(&self) -> &'static [ToolSpec] {
        TODO_SPECS
    }

    fn after_execute_hint(&self, tool_name: &str) -> AfterExecute {
        match tool_name {
            "todo.add" | "todo.update" => AfterExecute::EmitTodosUpdated,
            _ => AfterExecute::None,
        }
    }
}

#[derive(Deserialize)]
struct ListIn {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    category: Option<String>,
}

impl Tool for TodoListTool {
    fn name(&self) -> &str {
        "todo.list"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: ListIn = parse_tool_json(input, "todo.list").unwrap_or(ListIn {
            status: None,
            category: None,
        });
        let todos = self
            .db
            .list_todos(parsed.status.as_deref(), parsed.category.as_deref())
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let today = local_today();
        let with_overdue: Vec<_> = todos
            .iter()
            .map(|t| {
                json!({
                    "id": t.id,
                    "title": t.title,
                    "description": t.description,
                    "deadline": t.deadline,
                    "priority": t.priority,
                    "status": t.status,
                    "category": t.category,
                    "notes": t.notes,
                    "recurrence": t.recurrence,
                    "overdue": todo_is_overdue(t, &today),
                })
            })
            .collect();
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&with_overdue).unwrap_or_default(),
        })
    }
}

#[derive(Deserialize)]
struct AddIn {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    deadline: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    recurrence: Option<String>,
}

impl Tool for TodoAddTool {
    fn name(&self) -> &str {
        "todo.add"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: AddIn = parse_tool_json(input, "todo.add")?;
        let todo = self
            .db
            .upsert_todo(UpsertTodo {
                title: parsed.title,
                description: parsed.description,
                deadline: parsed.deadline,
                priority: parsed.priority,
                category: parsed.category,
                notes: parsed.notes,
                recurrence: parsed.recurrence,
                ..Default::default()
            })
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: format!("Added todo {} — {}", todo.id, todo.title),
        })
    }
}

#[derive(Deserialize)]
struct UpdateIn {
    id: String,
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    deadline: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

impl Tool for TodoUpdateTool {
    fn name(&self) -> &str {
        "todo.update"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: UpdateIn = parse_tool_json(input, "todo.update")?;
        match parsed.action.as_deref().unwrap_or("update") {
            "delete" => {
                self.db
                    .delete_todo(&parsed.id)
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok(ToolResult {
                    output: format!("Deleted todo {}", parsed.id),
                })
            }
            "complete" => {
                let todos = self
                    .db
                    .complete_todo(&parsed.id)
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok(ToolResult {
                    output: serde_json::to_string_pretty(&todos).unwrap_or_default(),
                })
            }
            _ => {
                let existing = self
                    .db
                    .get_todo(&parsed.id)
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                let todo = self
                    .db
                    .upsert_todo(UpsertTodo {
                        id: Some(parsed.id),
                        title: parsed.title.unwrap_or(existing.title),
                        description: parsed.description.or(existing.description),
                        deadline: parsed.deadline.or(existing.deadline),
                        priority: parsed.priority.or(Some(existing.priority)),
                        status: parsed.status.or(Some(existing.status)),
                        category: Some(existing.category),
                        notes: parsed.notes.or(existing.notes),
                        recurrence: Some(existing.recurrence),
                    })
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok(ToolResult {
                    output: serde_json::to_string_pretty(&todo).unwrap_or_default(),
                })
            }
        }
    }
}
