use std::sync::Arc;

use buddy_core::{Tool, ToolError, ToolResult};
use buddy_database::Database;
use serde::Deserialize;

pub struct SaveSparkTool {
    db: Arc<Database>,
}

pub struct UpdateSparkTool {
    db: Arc<Database>,
}

impl SaveSparkTool {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

impl UpdateSparkTool {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

#[derive(Debug, Deserialize)]
struct SaveSparkInput {
    content: String,
    tags: Vec<String>,
    #[serde(default)]
    source_conversation_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateSparkInput {
    id: String,
    action: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
}

impl Tool for SaveSparkTool {
    fn name(&self) -> &str {
        "save_spark"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: SaveSparkInput = serde_json::from_str(input).map_err(|e| {
            ToolError::ExecutionFailed(format!("save_spark expects JSON: {e}"))
        })?;

        if parsed.content.trim().is_empty() {
            return Err(ToolError::ExecutionFailed(
                "content must not be empty".into(),
            ));
        }

        let spark = self
            .db
            .create_spark(
                &parsed.content,
                &parsed.tags,
                parsed.source_conversation_id.as_deref(),
            )
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let tag_labels = spark.tags.join(", ");
        Ok(ToolResult {
            output: format!(
                "Saved spark {} with tags [{}]: {}",
                spark.id,
                tag_labels,
                spark.content
            ),
        })
    }
}

impl Tool for UpdateSparkTool {
    fn name(&self) -> &str {
        "update_spark"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: UpdateSparkInput = serde_json::from_str(input).map_err(|e| {
            ToolError::ExecutionFailed(format!("update_spark expects JSON: {e}"))
        })?;

        let spark = self
            .db
            .update_spark(
                &parsed.id,
                &parsed.action,
                parsed.content.as_deref(),
                parsed.tags.as_deref(),
            )
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(ToolResult {
            output: format!(
                "Spark {} {} — tags [{}]: {}",
                spark.id,
                parsed.action,
                spark.tags.join(", "),
                spark.content
            ),
        })
    }
}
