use buddy_core::{Tool, ToolDecl, ToolError, ToolResult};
use buddy_database::Database;
use buddy_memory::MemoryManager;
use buddy_plugins::{ExtraTool, PluginManager};
use std::sync::Arc;

struct PingTool;
impl Tool for PingTool {
    fn name(&self) -> &str {
        "ping.extra"
    }
    fn execute(&self, _input: &str) -> Result<ToolResult, ToolError> {
        Ok(ToolResult {
            output: "pong".into(),
        })
    }
}

#[test]
fn plugin_manager_registers_extras_without_app_changes() {
    let dir = std::env::temp_dir().join(format!("buddy-pm-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Arc::new(Database::open(&dir.join("buddy.db")).unwrap());
    let memory = Arc::new(MemoryManager::new(db.clone()));

    let mut mgr = PluginManager::bootstrap(db, memory, dir.display().to_string());
    mgr.register_extra(vec![ExtraTool {
        tool: Arc::new(PingTool),
        decl: ToolDecl {
            name: "ping.extra",
            planner_line: "ping.extra: returns pong",
        },
        schema: None,
    }]);

    let (registry, surface) = mgr.finish();
    assert!(surface.catalog.contains("ping.extra"));
    let out = registry.get("ping.extra").unwrap().execute("{}").unwrap();
    assert_eq!(out.output, "pong");
    let _ = std::fs::remove_dir_all(dir);
}
