use buddy_core::{AskKind, FieldSpec, Tool, ToolError, ToolResult, ToolSchema, ToolSpec};

pub struct EchoTool;

const ECHO_FIELDS: &[FieldSpec] = &[FieldSpec {
    name: "text",
    label: "text",
    required: true,
    memory_keys: &[],
    ask_kind: AskKind::Text,
    choices: &[],
}];

pub const ECHO_SCHEMA: ToolSchema = ToolSchema {
    tool: "echo",
    fields: ECHO_FIELDS,
};

pub const ECHO_SPEC: ToolSpec = ToolSpec {
    name: "echo",
    description: "returns the input text verbatim",
    example: r#"echo text="hello""#,
    schema: ECHO_SCHEMA,
    aliases: &[],
    rest_field: Some("text"),
    safety: buddy_core::Safety::Immediate,
    respond: buddy_core::RespondMode::Passthrough,
    likely: &[],
    extract: None,
};

impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let output = if let Ok(v) = serde_json::from_str::<serde_json::Value>(input) {
            v.get("text")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| input.to_string())
        } else {
            input.to_string()
        };
        Ok(ToolResult { output })
    }
}
