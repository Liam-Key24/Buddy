pub mod echo;

use std::sync::Arc;

use buddy_core::ToolRegistry;

pub fn create_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(echo::EchoTool));
    registry
}
