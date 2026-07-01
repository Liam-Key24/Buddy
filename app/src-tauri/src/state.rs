use std::path::PathBuf;
use std::sync::Arc;

use buddy_core::TaskRunner;
use buddy_database::Database;
use buddy_intelligence::IntelligenceService;
use buddy_memory::MemoryManager;
use buddy_plugins::create_registry;
use tauri::{AppHandle, Manager};

const DEFAULT_BRAIN_URL: &str = "http://127.0.0.1:8002";
const DEFAULT_MLX_URL: &str = "http://127.0.0.1:8001";

pub struct AppState {
    pub db: Arc<Database>,
    pub memory_manager: Arc<MemoryManager>,
    pub intelligence: Arc<IntelligenceService>,
    pub task_runner: Arc<TaskRunner>,
    pub project_root: PathBuf,
}

impl AppState {
    pub fn new(db: Database, project_root: PathBuf) -> Self {
        let db = Arc::new(db);
        seed_default_settings(&db);

        let registry = Arc::new(create_registry());
        let task_runner = Arc::new(TaskRunner::new(registry));
        let memory_manager = Arc::new(MemoryManager::new(db.clone()));
        let brain_url = DEFAULT_BRAIN_URL.to_string();
        let intelligence = Arc::new(IntelligenceService::new(
            db.clone(),
            memory_manager.clone(),
            brain_url,
        ));
        Self {
            db,
            memory_manager,
            intelligence,
            task_runner,
            project_root,
        }
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

fn seed_default_settings(db: &Database) {
    let defaults = [
        ("brain_url", DEFAULT_BRAIN_URL),
        ("mlx_url", DEFAULT_MLX_URL),
    ];
    for (key, value) in defaults {
        if db.get_setting(key).ok().flatten().is_none() {
            let _ = db.set_setting(key, value);
        }
    }
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
    let dir = app
        .path()
        .app_data_dir()
        .expect("failed to resolve app data dir");
    dir.join("buddy.db")
}

pub fn logs_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/Logs/Buddy")
}
