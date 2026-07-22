use buddy_core::TaskRunner;
use buddy_database::Database;
use buddy_memory::MemoryManager;
use buddy_plugins::{create_registry, tool_catalog_text};
use std::sync::Arc;

#[test]
fn calendar_tools_appear_in_catalog() {
    let catalog = tool_catalog_text();
    assert!(catalog.contains("calendar.list_events"));
    assert!(catalog.contains("calendar.create_event"));
    assert!(catalog.contains("calendar.duplicate_event"));
    assert!(catalog.contains("calendar.get_today"));
}

#[test]
fn registry_builds_without_calendar_service_tools() {
    // Builtin registry does not include calendar tools until CalendarPlugin::install.
    let dir = std::env::temp_dir().join(format!("buddy-cal-plugin-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("buddy.db");
    let db = Arc::new(Database::open(&path).unwrap());
    let memory = Arc::new(MemoryManager::new(db.clone()));
    let registry = Arc::new(create_registry(db, memory, dir.display().to_string()));
    let runner = TaskRunner::new(registry);
    let err = runner.run("calendar.get_today", "{}").unwrap_err();
    assert!(err.to_string().contains("not found"));
    let _ = std::fs::remove_dir_all(dir);
}
