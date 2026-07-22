use serde::Deserialize;

use crate::error::ToolError;

/// Parses a tool's JSON input, wrapping the serde error with the tool name so
/// planner-facing error messages are consistent across every tool.
pub fn parse_tool_json<T: for<'de> Deserialize<'de>>(
    input: &str,
    tool: &str,
) -> Result<T, ToolError> {
    serde_json::from_str(input)
        .map_err(|e| ToolError::ExecutionFailed(format!("{tool} expects JSON: {e}")))
}
