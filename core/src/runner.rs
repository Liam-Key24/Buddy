use std::sync::Arc;
use std::time::Instant;

use tracing::{info, instrument};

use crate::error::{ToolError, ToolResult};
use crate::registry::ToolRegistry;

pub struct TaskRunner {
    registry: Arc<ToolRegistry>,
}

impl TaskRunner {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }

    #[instrument(skip(self), fields(tool = %name))]
    pub fn run(&self, name: &str, input: &str) -> Result<ToolResult, ToolError> {
        let start = Instant::now();
        let tool = self.registry.get(name)?;
        let result = tool.execute(input)?;
        info!(
            duration_ms = start.elapsed().as_millis() as u64,
            "tool execution complete"
        );
        Ok(result)
    }
}
