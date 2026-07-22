pub mod echo;
pub mod external;
pub mod fs;
pub mod spark;

use std::sync::Arc;

use buddy_core::ToolRegistry;
use buddy_database::Database;

pub fn create_registry(db: Arc<Database>) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(echo::EchoTool));
    registry.register(Arc::new(spark::SaveSparkTool::new(db.clone())));
    registry.register(Arc::new(spark::UpdateSparkTool::new(db.clone())));

    registry.register(Arc::new(fs::ReadFileTool::new(db.clone())));
    registry.register(Arc::new(fs::WriteFileTool::new(db.clone())));
    registry.register(Arc::new(fs::EditFileTool::new(db.clone())));
    registry.register(Arc::new(fs::DeleteFileTool::new(db.clone())));
    registry.register(Arc::new(fs::ListDirTool::new(db.clone())));

    registry.register(Arc::new(external::SendEmailTool::new(db.clone())));
    registry.register(Arc::new(external::GitPushTool::new(db)));
    registry
}
