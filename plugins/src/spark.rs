use std::sync::Arc;

use buddy_core::{parse_tool_json, Tool, ToolError, ToolResult};
use buddy_database::Database;
use buddy_memory::{MemoryContext, MemoryEvent, MemoryManager};
use serde::Deserialize;

pub struct SaveSparkTool {
    db: Arc<Database>,
}

pub struct ListSparkTool {
    db: Arc<Database>,
}

pub struct UpdateSparkTool {
    db: Arc<Database>,
    memory: Arc<MemoryManager>,
    project_root: String,
}

impl SaveSparkTool {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

impl ListSparkTool {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

#[derive(Debug, Deserialize, Default)]
struct ListSparkInput {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

impl Tool for ListSparkTool {
    fn name(&self) -> &str {
        "list_sparks"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: ListSparkInput = parse_tool_json(input, "list_sparks").unwrap_or_default();
        let status = parsed
            .status
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let limit = parsed.limit.unwrap_or(30).clamp(1, 80);
        let sparks = self
            .db
            .list_sparks(status)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let rows: Vec<_> = sparks
            .into_iter()
            .take(limit)
            .map(|s| {
                let content = if s.content.chars().count() > 280 {
                    format!("{}…", s.content.chars().take(280).collect::<String>())
                } else {
                    s.content.clone()
                };
                serde_json::json!({
                    "id": s.id,
                    "content": content,
                    "tags": s.tags,
                    "status": s.status,
                    "updated_at": s.updated_at,
                })
            })
            .collect();
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&rows).unwrap_or_default(),
        })
    }
}

impl UpdateSparkTool {
    pub fn new(db: Arc<Database>, memory: Arc<MemoryManager>, project_root: String) -> Self {
        Self {
            db,
            memory,
            project_root,
        }
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
        let parsed: SaveSparkInput = parse_tool_json(input, "save_spark")?;

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
        let parsed: UpdateSparkInput = parse_tool_json(input, "update_spark")?;

        if parsed.action == "delete" {
            let spark = self
                .db
                .get_spark(&parsed.id)
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            // Memory owns archive-on-delete policy (sync fallback; no Brain).
            let tags = spark.tags.join(", ");
            let summary = if spark.content.len() > 500 {
                format!("{}...", &spark.content[..500])
            } else {
                spark.content.clone()
            };
            let ctx = MemoryContext {
                workspace_path: std::path::PathBuf::from(&self.project_root),
                conversation_id: spark.source_conversation_id.clone(),
                task_id: None,
            };
            let _ = self.memory.handle_event(
                &ctx,
                MemoryEvent::SparkArchivedSaved {
                    spark_id: spark.id.clone(),
                    content: spark.content.clone(),
                    tags: spark.tags.clone(),
                    summary: format!("Deleted spark [{tags}]: {summary}"),
                    topics: vec![],
                    key_facts: vec![],
                },
            );
        }

        let spark = self
            .db
            .update_spark(
                &parsed.id,
                &parsed.action,
                parsed.content.as_deref(),
                parsed.tags.as_deref(),
            )
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let msg = if parsed.action == "delete" {
            format!("Spark {} deleted and archived to memory", spark.id)
        } else {
            format!(
                "Spark {} {} — tags [{}]: {}",
                spark.id,
                parsed.action,
                spark.tags.join(", "),
                spark.content
            )
        };

        Ok(ToolResult { output: msg })
    }
}
