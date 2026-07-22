//! `coder.run` — Code Agent as a Core tool (single execution pipeline).

use std::sync::{Arc, Mutex};

use buddy_coder::{run_code_turn, CodeEmit};
use buddy_core::{parse_tool_json, FieldSpec, Tool, ToolDecl, ToolError, ToolResult, ToolSchema};
use buddy_database::Database;
use serde::Deserialize;
use tokio::runtime::Handle;

use crate::coder_bridge::KeychainSecrets;

struct CollectEmit {
    chunks: Mutex<String>,
}

impl CodeEmit for CollectEmit {
    fn chunk(&self, text: &str) {
        if let Ok(mut g) = self.chunks.lock() {
            g.push_str(text);
        }
    }
    fn error(&self, message: &str) {
        self.chunk(message);
    }
    fn preview_url(&self, url: &str) {
        self.chunk(&format!("\nPreview: {url}\n"));
    }
    fn done(&self) {}
}

pub struct CoderRunTool {
    db: Arc<Database>,
}

impl CoderRunTool {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
}

#[derive(Debug, Deserialize)]
struct CoderRunInput {
    conversation_id: String,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(alias = "text")]
    message: Option<String>,
    #[serde(default)]
    focus: Option<String>,
    #[serde(default)]
    attachments: Vec<String>,
}

impl Tool for CoderRunTool {
    fn name(&self) -> &str {
        "coder.run"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: CoderRunInput = parse_tool_json(input, "coder.run")?;
        let prompt = parsed
            .prompt
            .or(parsed.message)
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                // Generic session fallback — Buddy injects user_message for all tools.
                serde_json::from_str::<serde_json::Value>(input)
                    .ok()
                    .and_then(|v| {
                        v.get("user_message")
                            .and_then(|m| m.as_str())
                            .map(|s| s.to_string())
                    })
                    .filter(|s| !s.trim().is_empty())
            })
            .ok_or_else(|| ToolError::ExecutionFailed("prompt is required".into()))?;

        let db = self.db.clone();
        let conv = parsed.conversation_id.clone();
        let focus = parsed.focus.clone();
        let attachments = parsed.attachments.clone();
        let emit = Arc::new(CollectEmit {
            chunks: Mutex::new(String::new()),
        });
        let emit_run = emit.clone();

        let outcome = block_on(async move {
            run_code_turn(
                emit_run.as_ref(),
                &db,
                &KeychainSecrets,
                &conv,
                &prompt,
                focus.as_deref(),
                &attachments,
            )
            .await
        })?;

        let buffered = emit.chunks.lock().map(|g| g.clone()).unwrap_or_default();
        let output = if !outcome.assistant_content.trim().is_empty() {
            outcome.assistant_content
        } else {
            buffered
        };

        Ok(ToolResult { output })
    }
}

fn block_on<F, T>(fut: F) -> Result<T, ToolError>
where
    F: std::future::Future<Output = Result<T, String>>,
{
    let result = match Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fut)),
        Err(_) => tokio::runtime::Runtime::new()
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            .block_on(fut),
    };
    result.map_err(ToolError::ExecutionFailed)
}

pub const CODER_RUN_SCHEMA: ToolSchema = ToolSchema {
    tool: "coder.run",
    fields: &[
        FieldSpec {
            name: "conversation_id",
            label: "conversation",
            required: true,
            memory_keys: &[],
        },
        FieldSpec {
            name: "prompt",
            label: "coding request",
            required: true,
            memory_keys: &[],
        },
        FieldSpec {
            name: "focus",
            label: "focus mode",
            required: false,
            memory_keys: &[],
        },
    ],
};

pub fn coder_tool_decl() -> ToolDecl {
    ToolDecl {
        name: "coder.run",
        planner_line: "coder.run: run the Code Agent (Codex/Cursor) in the conversation workspace. tool_input JSON: {\"conversation_id\": \"...\", \"prompt\": \"...\", \"focus\": \"planning|asking|debugging|focused\"}.",
    }
}
