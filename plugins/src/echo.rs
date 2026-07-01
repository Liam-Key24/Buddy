use buddy_core::{Tool, ToolError, ToolResult};

pub struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        Ok(ToolResult {
            output: input.to_string(),
        })
    }
}
