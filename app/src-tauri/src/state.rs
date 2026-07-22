use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use buddy_calendar::CalendarService;
use buddy_core::{TaskRunner, ToolSchema};
use buddy_database::Database;
use buddy_intelligence::IntelligenceService;
use buddy_memory::MemoryManager;
use buddy_plugins::{seed_plugin_settings, ExtraTool, PluginManager, PluginSurface};
use tauri::{AppHandle, Manager};

use crate::calendar_bridge::DbSettings;
use crate::coder_tool::{self, CoderRunTool};
use crate::memory_api::MemoryApi;
use crate::memory_tools::{self, MemoryHandoverTool, MemoryMaintainTool, StateSlot};

const DEFAULT_BRAIN_URL: &str = "http://127.0.0.1:8002";
const DEFAULT_MLX_URL: &str = "http://127.0.0.1:8001";

pub struct AppState {
    pub db: Arc<Database>,
    pub memory: MemoryApi,
    pub task_runner: Arc<TaskRunner>,
    pub plugins: PluginSurface,
    pub calendar: Arc<CalendarService>,
    pub project_root: PathBuf,
    /// Kept for memory_extraction / checkers that still need the manager handle.
    pub memory_manager: Arc<MemoryManager>,
}

impl AppState {
    pub fn new(db: Database, project_root: PathBuf) -> Arc<Self> {
        let db = Arc::new(db);
        seed_default_settings(&db);

        let calendar = Arc::new(CalendarService::new(
            db.clone(),
            Arc::new(DbSettings { db: db.clone() }),
        ));

        let memory_manager = Arc::new(MemoryManager::new(db.clone()));
        let intelligence = Arc::new(IntelligenceService::new(
            db.clone(),
            memory_manager.clone(),
            DEFAULT_BRAIN_URL.to_string(),
        ));
        let memory = MemoryApi::new(
            memory_manager.clone(),
            intelligence,
            db.clone(),
            project_root.clone(),
        );

        let slot: StateSlot = Arc::new(OnceLock::new());

        let mut plugins = PluginManager::bootstrap(
            db.clone(),
            memory_manager.clone(),
            project_root.display().to_string(),
        );
        plugins.install_calendar(calendar.clone());
        plugins.register_extra(shell_extra_tools(db.clone(), slot.clone()));

        let (registry, surface) = plugins.finish();
        let task_runner = Arc::new(TaskRunner::new(Arc::new(registry)));

        let state = Arc::new(Self {
            db,
            memory,
            task_runner,
            plugins: surface,
            calendar,
            project_root,
            memory_manager,
        });
        let _ = slot.set(state.clone());
        state
    }

    pub fn tool_catalog_text(&self) -> &str {
        &self.plugins.catalog
    }

    pub fn tool_schema(&self, name: &str) -> Option<&'static ToolSchema> {
        self.plugins.schema(name)
    }

    pub fn brain_url(&self) -> String {
        self.db
            .get_setting("brain_url")
            .ok()
            .flatten()
            .unwrap_or_else(|| DEFAULT_BRAIN_URL.to_string())
    }

    pub fn mlx_url(&self) -> String {
        self.db
            .get_setting("mlx_url")
            .ok()
            .flatten()
            .unwrap_or_else(|| DEFAULT_MLX_URL.to_string())
    }
}

fn shell_extra_tools(db: Arc<Database>, slot: StateSlot) -> Vec<ExtraTool> {
    let mut extras = vec![ExtraTool {
        tool: Arc::new(CoderRunTool::new(db)),
        decl: coder_tool::coder_tool_decl(),
        schema: Some(&coder_tool::CODER_RUN_SCHEMA),
    }];
    let decls = memory_tools::memory_tool_decls();
    extras.push(ExtraTool {
        tool: Arc::new(MemoryHandoverTool::new(slot.clone())),
        decl: decls[0],
        schema: Some(&memory_tools::MEMORY_SCHEMAS[0]),
    });
    extras.push(ExtraTool {
        tool: Arc::new(MemoryMaintainTool::new(slot)),
        decl: decls[1],
        schema: Some(&memory_tools::MEMORY_SCHEMAS[1]),
    });
    extras
}

const SHELL_SETTING_DEFAULTS: &[(&str, &str)] = &[
    ("brain_url", DEFAULT_BRAIN_URL),
    ("mlx_url", DEFAULT_MLX_URL),
    ("codex_model", "gpt-5.5"),
    ("code_agent_backend", "cursor"),
    ("code_model", "auto"),
    (
        "personality_profile_json",
        r#"{"name":"Buddy","tone":"friendly","verbosity":"concise","humour":"low","confidence":"high","proactive":true,"uses_analogies":true,"uses_emojis":false}"#,
    ),
    ("clarification_confidence_threshold", "0.75"),
];

fn seed_default_settings(db: &Database) {
    for (key, value) in SHELL_SETTING_DEFAULTS {
        if db.get_setting(key).ok().flatten().is_none() {
            let _ = db.set_setting(key, value);
        }
    }
    seed_plugin_settings(db);
}

pub fn find_project_root() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..6 {
        if dir.join("brain").exists() && dir.join("app").exists() {
            return dir;
        }
        if dir.join("Cargo.toml").exists() && dir.join("brain").exists() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub fn db_path(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_data_dir().unwrap_or_else(|_| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Application Support/Buddy")
    });
    let _ = std::fs::create_dir_all(&dir);
    dir.join("buddy.db")
}

pub fn logs_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/Logs/Buddy")
}
