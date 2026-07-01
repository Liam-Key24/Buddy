use crate::error::{ToolError, ToolResult};

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError>;
}
