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
        for spec in &self.extra_specs {
            lines.push('\n');
            lines.push_str("- ");
            lines.push_str(&spec.planner_line());
        }
        if self.extra_specs.is_empty() {
            for decl in &self.extra_decls {
                lines.push('\n');
                lines.push_str("- ");
                lines.push_str(decl.planner_line);
            }
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

    /// OpenAI `tools=` function schemas derived from ToolSpec / ResolvedSpec.
    pub fn openai_tools(&self) -> Vec<serde_json::Value> {
        self.specs
            .iter()
            .filter(|spec| spec.name != "echo")
            .map(|spec| spec.openai_tool())
            .collect()
    }

    pub fn specs(&self) -> &[ResolvedSpec] {
        &self.specs
    }

    pub fn route(&self, text: &str) -> buddy_core::Route {
        buddy_core::route(text, &self.specs)
    }

    pub fn route_kind(&self, text: &str) -> buddy_core::RouteKind {
        buddy_core::route_kind(text, &self.specs)
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
            let mut resolved = ResolvedSpec::from_spec(spec);
            if resolved
                .schema
                .map(|schema| schema.fields.is_empty())
                .unwrap_or(true)
            {
                if let Some(schema) = plugin.tool_schemas().iter().find(|s| s.tool == spec.name) {
                    resolved.schema = Some(schema);
                }
            }
            by_name.insert(spec.name, resolved);
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
