pub mod calendar;
pub mod echo;
pub mod external;
pub mod fs;
pub mod manager;
pub mod spark;

use std::sync::Arc;

use buddy_core::{AfterExecute, BuddyPlugin, SettingSeed, Tool, ToolDecl, ToolRegistry, ToolSchema};
use buddy_database::Database;
use buddy_memory::MemoryManager;

pub struct EchoPlugin;
pub struct SparkPlugin;
pub struct FsPlugin;
pub struct ExternalPlugin;
pub use calendar::CalendarPlugin;
pub use manager::{seed_plugin_settings, ExtraTool, PluginManager, PluginSurface};

impl BuddyPlugin for EchoPlugin {
    fn id(&self) -> &'static str {
        "echo"
    }

    fn tools(&self, _db: Arc<Database>) -> Vec<Arc<dyn Tool>> {
        vec![Arc::new(echo::EchoTool)]
    }

    fn tool_decls(&self) -> &'static [ToolDecl] {
        &[ToolDecl {
            name: "echo",
            planner_line: "echo: returns the input text verbatim. Use when the user asks to echo something or says \"echo <text>\".",
        }]
    }
}

impl BuddyPlugin for SparkPlugin {
    fn id(&self) -> &'static str {
        "spark"
    }

    fn tools(&self, db: Arc<Database>) -> Vec<Arc<dyn Tool>> {
        // Save only here; UpdateSparkTool needs Memory and is registered in create_registry.
        vec![Arc::new(spark::SaveSparkTool::new(db))]
    }

    fn tool_decls(&self) -> &'static [ToolDecl] {
        &[
            ToolDecl {
                name: "save_spark",
                planner_line: "save_spark: saves a note or idea to Spark. tool_input must be JSON: {\"content\": \"<idea text>\", \"tags\": [\"<tag>\", ...]}",
            },
            ToolDecl {
                name: "update_spark",
                planner_line: "update_spark: updates an existing spark. tool_input must be JSON: {\"id\": \"<spark id>\", \"action\": \"respark\"|\"archive\"|\"edit\"|\"delete\", \"content\": \"<optional>\", \"tags\": [\"<optional>\"]}",
            },
        ]
    }

    fn tool_schemas(&self) -> &'static [buddy_core::ToolSchema] {
        &[
            buddy_core::ToolSchema {
                tool: "save_spark",
                fields: &[
                    buddy_core::FieldSpec {
                        name: "content",
                        label: "idea",
                        required: true,
                        memory_keys: &[],
                    },
                    buddy_core::FieldSpec {
                        name: "tags",
                        label: "tags",
                        required: false,
                        memory_keys: &[],
                    },
                ],
            },
            buddy_core::ToolSchema {
                tool: "update_spark",
                fields: &[
                    buddy_core::FieldSpec {
                        name: "id",
                        label: "spark",
                        required: true,
                        memory_keys: &[],
                    },
                    buddy_core::FieldSpec {
                        name: "action",
                        label: "action",
                        required: true,
                        memory_keys: &[],
                    },
                ],
            },
        ]
    }

    fn after_execute_hint(&self, tool_name: &str) -> AfterExecute {
        match tool_name {
            "save_spark" | "update_spark" => AfterExecute::EmitSparksUpdated,
            _ => AfterExecute::None,
        }
    }
}

impl BuddyPlugin for FsPlugin {
    fn id(&self) -> &'static str {
        "fs"
    }

    fn tools(&self, db: Arc<Database>) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(fs::ReadFileTool::new(db.clone())),
            Arc::new(fs::WriteFileTool::new(db.clone())),
            Arc::new(fs::EditFileTool::new(db.clone())),
            Arc::new(fs::DeleteFileTool::new(db.clone())),
            Arc::new(fs::ListDirTool::new(db)),
        ]
    }

    fn tool_decls(&self) -> &'static [ToolDecl] {
        &[
            ToolDecl {
                name: "read_file",
                planner_line: "read_file: read a file's contents. tool_input must be JSON: {\"path\": \"<path>\"}. Paths are relative to the user's home folder or absolute within it.",
            },
            ToolDecl {
                name: "write_file",
                planner_line: "write_file: create or overwrite a file. tool_input must be JSON: {\"path\": \"<path>\", \"content\": \"<full file contents>\"}",
            },
            ToolDecl {
                name: "edit_file",
                planner_line: "edit_file: edit an existing file. tool_input must be JSON: {\"path\": \"<path>\", \"old\": \"<text to find>\", \"new\": \"<replacement>\"} for a targeted change, or {\"path\": \"<path>\", \"content\": \"<full new contents>\"} to replace the whole file.",
            },
            ToolDecl {
                name: "delete_file",
                planner_line: "delete_file: delete a file. tool_input must be JSON: {\"path\": \"<path>\"}",
            },
            ToolDecl {
                name: "list_dir",
                planner_line: "list_dir: list a directory. tool_input must be JSON: {\"path\": \"<path>\", \"depth\": <optional number>}",
            },
        ]
    }

    fn setting_seeds(&self) -> &'static [SettingSeed] {
        &[SettingSeed {
            key: "fs_excluded_paths",
            value: r#"["Library",".Trash",".ssh",".gnupg",".cache","Pictures"]"#,
        }]
    }
}

impl BuddyPlugin for ExternalPlugin {
    fn id(&self) -> &'static str {
        "external"
    }

    fn tools(&self, db: Arc<Database>) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(external::SendEmailTool::new(db.clone())),
            Arc::new(external::GitPushTool::new(db)),
        ]
    }

    fn tool_decls(&self) -> &'static [ToolDecl] {
        &[
            ToolDecl {
                name: "send_email",
                planner_line: "send_email: draft an email using the user's saved templates. tool_input must be JSON: {\"to\": \"<address>\", \"subject\": \"<subject>\", \"body\": \"<body>\", \"name\": \"<optional recipient name>\"}. Emails are drafted for approval, not sent automatically.",
            },
            ToolDecl {
                name: "git_push",
                planner_line: "git_push: request pushing a repo to its remote. tool_input must be JSON: {\"remote\": \"<optional>\", \"branch\": \"<optional>\", \"repo_path\": \"<optional>\"}. Requires user approval.",
            },
        ]
    }

    fn tool_schemas(&self) -> &'static [ToolSchema] {
        &[ToolSchema {
            tool: "send_email",
            fields: &[
                buddy_core::FieldSpec {
                    name: "to",
                    label: "recipient",
                    required: true,
                    memory_keys: &[],
                },
                buddy_core::FieldSpec {
                    name: "body",
                    label: "message",
                    required: true,
                    memory_keys: &[],
                },
                buddy_core::FieldSpec {
                    name: "subject",
                    label: "subject",
                    required: false,
                    memory_keys: &[],
                },
                buddy_core::FieldSpec {
                    name: "name",
                    label: "recipient name",
                    required: false,
                    memory_keys: &[],
                },
            ],
        }]
    }

    fn setting_seeds(&self) -> &'static [SettingSeed] {
        &[
            SettingSeed {
                key: "email_greeting",
                value: "Hi,",
            },
            SettingSeed {
                key: "email_signature",
                value: "",
            },
            SettingSeed {
                key: "email_body_template",
                value: "{greeting}\n\n{body}\n\n{signature}",
            },
        ]
    }

    fn secret_keys(&self) -> &'static [&'static str] {
        &["smtp_password"]
    }
}

/// All compile-time builtin plugins, in registration order. Adding a new
/// builtin capability means implementing `BuddyPlugin` and adding it here —
/// the tool registry, settings seeding, secrets allowlist and planner tool
/// list all derive from this list.
pub fn all_builtin_plugins() -> Vec<Box<dyn BuddyPlugin>> {
    vec![
        Box::new(EchoPlugin),
        Box::new(SparkPlugin),
        Box::new(FsPlugin),
        Box::new(ExternalPlugin),
        Box::new(CalendarPlugin),
    ]
}

pub fn create_registry(
    db: Arc<Database>,
    memory: Arc<MemoryManager>,
    project_root: impl Into<String>,
) -> ToolRegistry {
    let project_root = project_root.into();
    let mut registry = ToolRegistry::new();
    for plugin in all_builtin_plugins() {
        for tool in plugin.tools(db.clone()) {
            registry.register(tool);
        }
    }
    // Spark update/delete archives via Memory — needs MemoryManager.
    registry.register(Arc::new(spark::UpdateSparkTool::new(
        db,
        memory,
        project_root,
    )));
    registry
}

/// Looks up the `AfterExecute` hint for a tool by asking every builtin
/// plugin, so the orchestrator doesn't need to hardcode tool names to know
/// which ones require a post-execution side effect (e.g. re-emitting spark
/// events).
pub fn after_execute_hint(tool_name: &str) -> AfterExecute {
    all_builtin_plugins()
        .iter()
        .map(|plugin| plugin.after_execute_hint(tool_name))
        .find(|hint| *hint != AfterExecute::None)
        .unwrap_or(AfterExecute::None)
}

/// Planner-facing tool catalog built from every builtin plugin's
/// `tool_decls()`, so the Brain's "available tools" list stays in sync with
/// the actual registry without a hand-maintained copy.
pub fn tool_catalog_text() -> String {
    all_builtin_plugins()
        .iter()
        .flat_map(|plugin| plugin.tool_decls().iter())
        .map(|decl| format!("- {}", decl.planner_line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Clarification schemas from every builtin plugin.
pub fn tool_schema(tool_name: &str) -> Option<&'static ToolSchema> {
    all_builtin_plugins()
        .iter()
        .flat_map(|plugin| plugin.tool_schemas().iter())
        .find(|schema| schema.tool == tool_name)
}
