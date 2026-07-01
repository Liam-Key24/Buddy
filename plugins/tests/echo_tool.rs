use std::sync::Arc;

use buddy_core::TaskRunner;
use buddy_plugins::create_registry;

#[test]
fn echo_tool_returns_input() {
    let registry = Arc::new(create_registry());
    let runner = TaskRunner::new(registry);

    let result = runner.run("echo", "hello").unwrap();
    assert_eq!(result.output, "hello");
}
