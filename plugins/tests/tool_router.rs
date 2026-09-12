use std::sync::Arc;

use buddy_core::Route;
use buddy_database::Database;
use buddy_memory::MemoryManager;
use buddy_plugins::PluginManager;
use serde_json::Value;

fn surface() -> (tempfile_dir::Guard, buddy_plugins::PluginSurface) {
    let dir = std::env::temp_dir().join(format!("buddy-router-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Arc::new(Database::open(&dir.join("buddy.db")).unwrap());
    let memory = Arc::new(MemoryManager::new(db.clone()));
    let mgr = PluginManager::bootstrap(db, memory, dir.display().to_string());
    let (_registry, surface) = mgr.finish();
    (tempfile_dir::Guard(dir), surface)
}

mod tempfile_dir {
    pub struct Guard(pub std::path::PathBuf);
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[test]
fn canonical_todo_add_hits_without_llm() {
    let (_dir, surface) = surface();
    match surface.route(r#"todo.add title="Buy milk" deadline=2026-08-18"#) {
        Route::Tools(jobs) => {
            assert_eq!(jobs[0].tool, "todo.add");
            let v: Value = serde_json::from_str(&jobs[0].input).unwrap();
            assert_eq!(v["title"], "Buy milk");
            assert_eq!(v["deadline"], "2026-08-18");
        }
        Route::Miss => panic!("expected canonical hit"),
    }
}

#[test]
fn echo_rest_field_and_json_envelope() {
    let (_dir, surface) = surface();
    match surface.route("echo hello buddy") {
        Route::Tools(jobs) => {
            assert_eq!(jobs[0].tool, "echo");
            let v: Value = serde_json::from_str(&jobs[0].input).unwrap();
            assert_eq!(v["text"], "hello buddy");
        }
        Route::Miss => panic!("expected echo"),
    }
    match surface.route(r#"{"tool":"echo","text":"ping"}"#) {
        Route::Tools(jobs) => {
            let v: Value = serde_json::from_str(&jobs[0].input).unwrap();
            assert_eq!(v["text"], "ping");
        }
        Route::Miss => panic!("expected json echo"),
    }
}

#[test]
fn nl_todo_extract_and_chat_miss() {
    let (_dir, surface) = surface();
    match surface.route("remind me to call the dentist") {
        Route::Tools(jobs) => {
            assert_eq!(jobs[0].tool, "todo.add");
            let v: Value = serde_json::from_str(&jobs[0].input).unwrap();
            assert_eq!(v["title"], "call the dentist");
        }
        Route::Miss => panic!("expected extract"),
    }
    assert!(matches!(
        surface.route("whats the capital of the uk"),
        Route::Miss
    ));
}

#[test]
fn synthesized_calendar_look_kv() {
    let (_dir, surface) = surface();
    match surface.route("calendar.look when=today focus=free") {
        Route::Tools(jobs) => {
            assert_eq!(jobs[0].tool, "calendar.look");
            let v: Value = serde_json::from_str(&jobs[0].input).unwrap();
            assert_eq!(v["when"], "today");
            assert_eq!(v["focus"], "free");
        }
        Route::Miss => panic!("expected synthesized look"),
    }
}
