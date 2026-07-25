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
    ("auto_start_mlx", "true"),
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

fn looks_like_project_root(dir: &std::path::Path) -> bool {
    dir.join("brain").is_dir()
        && (dir.join("app").is_dir() || dir.join("Cargo.toml").is_file())
}

fn walk_up_for_root(start: PathBuf) -> Option<PathBuf> {
    let mut dir = start;
    for _ in 0..8 {
        if looks_like_project_root(&dir) {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn candidate_home_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Some(home) = dirs::home_dir() else {
        return out;
    };
    for rel in [
        "Desktop/BUDDY",
        "Desktop/Buddy",
        "Documents/BUDDY",
        "Documents/Buddy",
        "Developer/BUDDY",
        "dev/BUDDY",
        "BUDDY",
        "Buddy",
        "src/BUDDY",
    ] {
        out.push(home.join(rel));
    }
    out
}

/// Discover the repo root used for Brain/MLX (venv + scripts).
/// Preference: env → DB setting → cwd walk → executable walk → common home paths.
pub fn resolve_project_root(db: &Database) -> PathBuf {
    if let Ok(env_root) = std::env::var("BUDDY_PROJECT_ROOT") {
        let path = PathBuf::from(env_root.trim());
        if looks_like_project_root(&path) {
            let _ = db.set_setting("project_root", &path.display().to_string());
            return path;
        }
    }

    if let Ok(Some(stored)) = db.get_setting("project_root") {
        let path = PathBuf::from(stored.trim());
        if looks_like_project_root(&path) {
            return path;
        }
    }

    let discovered = find_project_root();
    if looks_like_project_root(&discovered) {
        let _ = db.set_setting("project_root", &discovered.display().to_string());
        return discovered;
    }

    discovered
}

pub fn find_project_root() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(root) = walk_up_for_root(cwd) {
            return root;
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            if let Some(root) = walk_up_for_root(parent.to_path_buf()) {
                return root;
            }
        }
    }

    for candidate in candidate_home_roots() {
        if looks_like_project_root(&candidate) {
            return candidate;
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
