//! Central plugin registration, catalog, and schema lookup.
//!
//! AppState installs capabilities once through [`PluginManager`] — adding a
//! builtin plugin means registering it in [`crate::all_builtin_plugins`] (and
//! calling an install hook only when the plugin needs extra services).

use std::sync::Arc;

use buddy_calendar::CalendarService;
use buddy_core::{
    AfterExecute, ResolvedSpec, Tool, ToolDecl, ToolError, ToolRegistry, ToolResult, ToolSchema,
};
use buddy_database::Database;
use buddy_memory::MemoryManager;

use crate::{
    after_execute_hint as plugin_after_hint, all_builtin_plugins, create_registry, CalendarPlugin,
    SocialsPlugin,
};

/// Extra tools contributed by the app shell (coder, memory) that need services
/// beyond `Database`. Registered through the manager so AppState stays declarative.
pub struct ExtraTool {
    pub tool: Arc<dyn Tool>,
    pub decl: ToolDecl,
    pub schema: Option<&'static ToolSchema>,
    pub spec: Option<&'static buddy_core::ToolSpec>,
}

/// Owns the tool registry plus planner catalog / clarification schemas.
pub struct PluginManager {
    registry: ToolRegistry,
    extra_decls: Vec<ToolDecl>,
    extra_schemas: Vec<&'static ToolSchema>,
    extra_specs: Vec<&'static buddy_core::ToolSpec>,
}

impl PluginManager {
    /// Register all builtin plugins (fs, spark, echo, external, calendar decls).
    pub fn bootstrap(
        db: Arc<Database>,
        memory: Arc<MemoryManager>,
        project_root: impl Into<String>,
    ) -> Self {
        Self {
            registry: create_registry(db, memory, project_root),
            extra_decls: Vec::new(),
            extra_schemas: Vec::new(),
            extra_specs: Vec::new(),
        }
    }

    /// Calendar/lifestyle executors need CalendarService.
    pub fn install_calendar(&mut self, service: Arc<CalendarService>) {
        CalendarPlugin::install(&mut self.registry, service);
    }

    /// Socials commit pins approved posts via CalendarService.
    pub fn install_socials(&mut self, db: Arc<Database>, service: Arc<CalendarService>) {
        SocialsPlugin::install(&mut self.registry, db, service);
    }

    /// Register shell-provided tools (coder, memory.*) without AppState branching later.
    pub fn register_extra(&mut self, extras: Vec<ExtraTool>) {
        for extra in extras {
            self.registry.register(extra.tool);
            self.extra_decls.push(extra.decl);
            if let Some(schema) = extra.schema {
                self.extra_schemas.push(schema);
            }
            if let Some(spec) = extra.spec {
                self.extra_specs.push(spec);
            }
        }
    }

    pub fn catalog_text(&self) -> String {
        let mut lines = crate::tool_catalog_text();
        for decl in &self.extra_decls {
            lines.push('\n');
            lines.push_str("- ");
            lines.push_str(decl.planner_line);
        }
        lines
    }

    pub fn schema(&self, tool_name: &str) -> Option<&'static ToolSchema> {
        if let Some(s) = crate::tool_schema(tool_name) {
            return Some(s);
        }
        self.extra_schemas
            .iter()
            .copied()
            .find(|s| s.tool == tool_name)
    }

    pub fn after_execute_hint(&self, tool_name: &str) -> AfterExecute {
        plugin_after_hint(tool_name)
    }

    /// Split into executable registry + catalog/schema surface for AppState.
    pub fn finish(self) -> (ToolRegistry, PluginSurface) {
        let catalog = self.catalog_text();
        let specs =
            collect_resolved_specs(&self.extra_decls, &self.extra_schemas, &self.extra_specs);
        (
            self.registry,
            PluginSurface {
                catalog,
                extra_schemas: self.extra_schemas,
                extra_decls: self.extra_decls,
                specs,
            },
        )
    }

    pub fn into_registry(self) -> ToolRegistry {
        self.finish().0
    }

    pub fn run(&self, name: &str, input: &str) -> Result<ToolResult, ToolError> {
        let tool = self.registry.get(name)?;
        tool.execute(input)
    }
}

/// Planner catalog + clarification schemas after plugins are installed.
pub struct PluginSurface {
    pub catalog: String,
    extra_schemas: Vec<&'static ToolSchema>,
    extra_decls: Vec<ToolDecl>,
    specs: Vec<ResolvedSpec>,
}

impl PluginSurface {
    pub fn schema(&self, tool_name: &str) -> Option<&'static ToolSchema> {
        if let Some(s) = crate::tool_schema(tool_name) {
            return Some(s);
        }
        self.extra_schemas
            .iter()
            .copied()
            .find(|s| s.tool == tool_name)
    }

    /// OpenAI `tools=` function schemas (JSON Schema from decls + FieldSpec).
    pub fn openai_tools(&self) -> Vec<serde_json::Value> {
        let mut decls: Vec<ToolDecl> = crate::all_builtin_plugins()
            .iter()
            .flat_map(|plugin| plugin.tool_decls().iter().copied())
            .collect();
        decls.extend(self.extra_decls.iter().copied());
        decls
            .into_iter()
            .filter(|decl| decl.name != "echo")
            .map(|decl| openai_tool_from_decl(&decl, self.schema(decl.name)))
            .collect()
    }

    pub fn specs(&self) -> &[ResolvedSpec] {
        &self.specs
    }

    pub fn route(&self, text: &str) -> buddy_core::Route {
        buddy_core::route(text, &self.specs)
    }
}

fn collect_resolved_specs(
    extra_decls: &[ToolDecl],
    extra_schemas: &[&'static ToolSchema],
    extra_specs: &[&'static buddy_core::ToolSpec],
) -> Vec<ResolvedSpec> {
    let mut by_name: std::collections::HashMap<&'static str, ResolvedSpec> =
        std::collections::HashMap::new();
    for plugin in crate::all_builtin_plugins() {
        for spec in plugin.tool_specs() {
            by_name.insert(spec.name, ResolvedSpec::from_spec(spec));
        }
        for decl in plugin.tool_decls() {
            let schema = plugin.tool_schemas().iter().find(|s| s.tool == decl.name);
            by_name
                .entry(decl.name)
                .or_insert_with(|| ResolvedSpec::from_decl(*decl, schema));
        }
    }
    for spec in extra_specs {
        by_name.insert(spec.name, ResolvedSpec::from_spec(spec));
    }
    for decl in extra_decls {
        let schema = extra_schemas.iter().copied().find(|s| s.tool == decl.name);
        by_name
            .entry(decl.name)
            .or_insert_with(|| ResolvedSpec::from_decl(*decl, schema));
    }
    by_name.into_values().collect()
}

fn openai_tool_from_decl(
    decl: &ToolDecl,
    schema: Option<&'static ToolSchema>,
) -> serde_json::Value {
    let description = decl
        .planner_line
        .split_once(':')
        .map(|(_, rest)| rest.trim())
        .unwrap_or(decl.planner_line);
    let mut properties = serde_json::Map::new();
    if let Some(schema) = schema {
        for field in schema.fields {
            properties.insert(
                field.name.to_string(),
                serde_json::json!({
                    "description": field.label,
                }),
            );
        }
    }
    // Calendar native tools: richer shapes than FieldSpec labels.
    if decl.name == "calendar.look" {
        properties.insert(
            "when".into(),
            serde_json::json!({"type": "string", "description": "today | tomorrow | this_week | next_week | not_today | weekend | YYYY-MM-DD"}),
        );
        properties.insert(
            "focus".into(),
            serde_json::json!({"type": "string", "description": "events | free | work"}),
        );
        properties.insert(
            "duration_minutes".into(),
            serde_json::json!({"type": "integer", "description": "slot length when focus=free"}),
        );
        properties.insert(
            "query".into(),
            serde_json::json!({"type": "string", "description": "optional title filter"}),
        );
    } else if decl.name == "calendar.pin" {
        properties.insert(
            "action".into(),
            serde_json::json!({"type": "string", "description": "create | update | delete"}),
        );
        properties.insert("title".into(), serde_json::json!({"type": "string"}));
        properties.insert(
            "start".into(),
            serde_json::json!({"type": "string", "description": "ISO or 'tomorrow 14:00'"}),
        );
        properties.insert("end".into(), serde_json::json!({"type": "string"}));
        properties.insert("id".into(), serde_json::json!({"type": "string"}));
        properties.insert("force".into(), serde_json::json!({"type": "boolean"}));
    } else if decl.name == "calendar.organize" {
        properties.insert(
            "window".into(),
            serde_json::json!({"type": "string", "description": "this_week | next_week | today | tomorrow | weekend | sunday"}),
        );
        properties.insert(
            "mode".into(),
            serde_json::json!({"type": "string", "description": "propose | commit"}),
        );
        properties.insert(
            "constraints".into(),
            serde_json::json!({"type": "array", "items": {"type": "string"}}),
        );
        properties.insert(
            "items".into(),
            serde_json::json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "title": {"type": "string"},
                        "duration": {"type": "string"},
                        "duration_minutes": {"type": "integer"},
                        "when": {"type": "string"},
                        "count": {"type": "integer"}
                    }
                }
            }),
        );
    } else if decl.name == "docs.upsert" {
        properties.insert("title".into(), serde_json::json!({"type": "string"}));
        properties.insert(
            "content".into(),
            serde_json::json!({
                "type": "string",
                "description": "markdown body; pass the user's paste through with only light cleanup"
            }),
        );
        properties.insert(
            "id".into(),
            serde_json::json!({"type": "string", "description": "id or existing title"}),
        );
        properties.insert(
            "format".into(),
            serde_json::json!({"type": "string", "description": "markdown | html | csv"}),
        );
    } else if decl.name == "docs.format" {
        properties.insert(
            "id".into(),
            serde_json::json!({"type": "string", "description": "id or existing title"}),
        );
        properties.insert("title".into(), serde_json::json!({"type": "string"}));
    } else if decl.name == "docs.patch" {
        properties.insert(
            "id".into(),
            serde_json::json!({"type": "string", "description": "id or existing title"}),
        );
        properties.insert("find".into(), serde_json::json!({"type": "string"}));
        properties.insert("replace".into(), serde_json::json!({"type": "string"}));
    } else if decl.name == "fitness.look" {
        properties.insert(
            "what".into(),
            serde_json::json!({"type": "string", "description": "food | workouts | weight | climbs | prs | fridge | recipes | summary"}),
        );
        properties.insert(
            "date".into(),
            serde_json::json!({"type": "string", "description": "YYYY-MM-DD for food"}),
        );
        properties.insert("limit".into(), serde_json::json!({"type": "integer"}));
    } else if decl.name == "fitness.log_workout" {
        properties.insert("name".into(), serde_json::json!({"type": "string"}));
        properties.insert("date".into(), serde_json::json!({"type": "string"}));
        properties.insert(
            "sets".into(),
            serde_json::json!({
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "exercise": {"type": "string"},
                        "reps": {"type": "integer"},
                        "weight": {"type": "number"}
                    }
                }
            }),
        );
    } else if decl.name == "money.list" {
        properties.insert("year".into(), serde_json::json!({"type": "integer"}));
        properties.insert("month".into(), serde_json::json!({"type": "integer"}));
        properties.insert(
            "kind".into(),
            serde_json::json!({"type": "string", "description": "expense | income"}),
        );
    } else if decl.name == "money.pot" {
        properties.insert(
            "name".into(),
            serde_json::json!({"type": "string", "description": "pot name e.g. holiday"}),
        );
        properties.insert(
            "amount".into(),
            serde_json::json!({"type": "number", "description": "pounds"}),
        );
        properties.insert(
            "mode".into(),
            serde_json::json!({"type": "string", "description": "set | add"}),
        );
    } else if decl.name == "money.pots" {
        // no args
    } else if decl.name == "study.look" {
        properties.insert(
            "what".into(),
            serde_json::json!({"type": "string", "description": "sessions | subjects | topics | assignments | status"}),
        );
        properties.insert("subject_id".into(), serde_json::json!({"type": "string"}));
        properties.insert("limit".into(), serde_json::json!({"type": "integer"}));
    } else if decl.name == "study.upsert_subject" {
        properties.insert("name".into(), serde_json::json!({"type": "string"}));
        properties.insert("id".into(), serde_json::json!({"type": "string"}));
        properties.insert("color".into(), serde_json::json!({"type": "string"}));
    } else if decl.name == "study.upsert_topic" {
        properties.insert("name".into(), serde_json::json!({"type": "string"}));
        properties.insert("subject_id".into(), serde_json::json!({"type": "string"}));
        properties.insert(
            "subject".into(),
            serde_json::json!({"type": "string", "description": "subject name if id unknown"}),
        );
        properties.insert("deadline".into(), serde_json::json!({"type": "string"}));
        properties.insert("priority".into(), serde_json::json!({"type": "string"}));
        properties.insert("notes".into(), serde_json::json!({"type": "string"}));
    } else if decl.name == "study.upsert_assignment" {
        properties.insert("title".into(), serde_json::json!({"type": "string"}));
        properties.insert("subject_id".into(), serde_json::json!({"type": "string"}));
        properties.insert("subject".into(), serde_json::json!({"type": "string"}));
        properties.insert("topic_id".into(), serde_json::json!({"type": "string"}));
        properties.insert("kind".into(), serde_json::json!({"type": "string"}));
        properties.insert("deadline".into(), serde_json::json!({"type": "string"}));
        properties.insert("priority".into(), serde_json::json!({"type": "string"}));
    } else if decl.name == "fitness.log_weight" {
        properties.insert("kg".into(), serde_json::json!({"type": "number"}));
        properties.insert("date".into(), serde_json::json!({"type": "string"}));
        properties.insert("notes".into(), serde_json::json!({"type": "string"}));
    } else if decl.name == "research.get" {
        properties.insert(
            "conversation_id".into(),
            serde_json::json!({"type": "string"}),
        );
        properties.insert("id".into(), serde_json::json!({"type": "string"}));
        properties.insert("query".into(), serde_json::json!({"type": "string"}));
    } else if decl.name == "socials.look" {
        properties.insert(
            "what".into(),
            serde_json::json!({"type": "string", "description": "ideas | drafts | threads | projects | published | plans | profile"}),
        );
    } else if decl.name == "list_sparks" {
        properties.insert(
            "status".into(),
            serde_json::json!({"type": "string", "description": "active | archived"}),
        );
        properties.insert("limit".into(), serde_json::json!({"type": "integer"}));
    }
    serde_json::json!({
        "type": "function",
        "function": {
            "name": decl.name,
            "description": description,
            "parameters": {
                "type": "object",
                "properties": properties,
                "additionalProperties": true
            }
        }
    })
}

/// Seeds settings from every builtin plugin (call once at startup).
pub fn seed_plugin_settings(db: &Database) {
    for plugin in all_builtin_plugins() {
        for seed in plugin.setting_seeds() {
            if db.get_setting(seed.key).ok().flatten().is_none() {
                let _ = db.set_setting(seed.key, seed.value);
            }
        }
    }
}
