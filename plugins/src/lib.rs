pub mod echo;
pub mod spark;

use std::sync::Arc;

use buddy_core::ToolRegistry;
use buddy_database::Database;

pub fn create_registry(db: Arc<Database>) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(echo::EchoTool));
    registry.register(Arc::new(spark::SaveSparkTool::new(db.clone())));
    registry.register(Arc::new(spark::UpdateSparkTool::new(db)));
    registry
}
