use std::sync::Arc;

use buddy_core::TaskRunner;
use buddy_database::Database;
use buddy_plugins::create_registry;

#[test]
fn echo_tool_returns_input() {
    let dir = std::env::temp_dir().join(format!("buddy-echo-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("buddy.db");
    let db = Arc::new(Database::open(&path).unwrap());
    let registry = Arc::new(create_registry(db));
    let runner = TaskRunner::new(registry);

    let result = runner.run("echo", "hello").unwrap();
    assert_eq!(result.output, "hello");

    let _ = std::fs::remove_dir_all(dir);
}
