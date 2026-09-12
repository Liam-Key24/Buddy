use buddy_core::TaskRunner;
use buddy_database::Database;
use buddy_memory::MemoryManager;
use buddy_plugins::{create_registry, tool_catalog_text};
use std::sync::Arc;

#[test]
fn calendar_tools_appear_in_catalog() {
    let catalog = tool_catalog_text();
    assert!(catalog.contains("calendar.look"));
    assert!(catalog.contains("calendar.pin"));
    assert!(catalog.contains("calendar.organize"));
    assert!(catalog.contains("money.log"));
    assert!(catalog.contains("money.pot"));
    assert!(catalog.contains("money.pots"));
    assert!(catalog.contains("fitness.log_food"));
    assert!(catalog.contains("fitness.look"));
    assert!(catalog.contains("money.list"));
    assert!(catalog.contains("study.look"));
    assert!(catalog.contains("research.get"));
    assert!(catalog.contains("socials.look"));
    assert!(catalog.contains("list_sparks"));
    assert!(catalog.contains("lifestyle.set_schedule"));
    assert!(!catalog.contains("calendar.schedule_task"));
    assert!(!catalog.contains("echo:"));
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
