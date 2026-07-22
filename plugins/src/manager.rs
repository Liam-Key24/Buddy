//! Central plugin registration, catalog, and schema lookup.
//!
//! AppState installs capabilities once through [`PluginManager`] — adding a
//! builtin plugin means registering it in [`crate::all_builtin_plugins`] (and
//! calling an install hook only when the plugin needs extra services).

use std::sync::Arc;

use buddy_calendar::CalendarService;
use buddy_core::{AfterExecute, Tool, ToolDecl, ToolRegistry, ToolSchema, ToolResult, ToolError};
use buddy_database::Database;
use buddy_memory::MemoryManager;

use crate::{
    all_builtin_plugins, after_execute_hint as plugin_after_hint, create_registry, CalendarPlugin,
};

/// Extra tools contributed by the app shell (coder, memory) that need services
/// beyond `Database`. Registered through the manager so AppState stays declarative.
pub struct ExtraTool {
    pub tool: Arc<dyn Tool>,
    pub decl: ToolDecl,
    pub schema: Option<&'static ToolSchema>,
}

/// Owns the tool registry plus planner catalog / clarification schemas.
pub struct PluginManager {
    registry: ToolRegistry,
    extra_decls: Vec<ToolDecl>,
    extra_schemas: Vec<&'static ToolSchema>,
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
        }
    }

    /// Calendar/lifestyle executors need CalendarService.
    pub fn install_calendar(&mut self, service: Arc<CalendarService>) {
        CalendarPlugin::install(&mut self.registry, service);
    }

    /// Register shell-provided tools (coder, memory.*) without AppState branching later.
    pub fn register_extra(&mut self, extras: Vec<ExtraTool>) {
        for extra in extras {
            self.registry.register(extra.tool);
            self.extra_decls.push(extra.decl);
            if let Some(schema) = extra.schema {
                self.extra_schemas.push(schema);
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
        (
            self.registry,
            PluginSurface {
                catalog,
                extra_schemas: self.extra_schemas,
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
