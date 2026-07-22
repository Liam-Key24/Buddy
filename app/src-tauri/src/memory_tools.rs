//! Core tools for Memory operations (handover / maintenance).

use std::sync::{Arc, OnceLock};

use buddy_core::{parse_tool_json, FieldSpec, Tool, ToolDecl, ToolError, ToolResult, ToolSchema};
use serde::Deserialize;
use tokio::runtime::Handle;

use crate::state::AppState;

pub type StateSlot = Arc<OnceLock<Arc<AppState>>>;

pub struct MemoryHandoverTool {
    slot: StateSlot,
}

pub struct MemoryMaintainTool {
    slot: StateSlot,
}

impl MemoryHandoverTool {
    pub fn new(slot: StateSlot) -> Self {
        Self { slot }
    }
}

impl MemoryMaintainTool {
    pub fn new(slot: StateSlot) -> Self {
        Self { slot }
    }
}

#[derive(Debug, Deserialize)]
struct MemoryConvInput {
    conversation_id: String,
}

fn state(slot: &StateSlot) -> Result<Arc<AppState>, ToolError> {
    slot.get()
        .cloned()
        .ok_or_else(|| ToolError::ExecutionFailed("app state not ready".into()))
}

fn block_on_async<F, T>(fut: F) -> Result<T, ToolError>
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

impl Tool for MemoryHandoverTool {
    fn name(&self) -> &str {
        "memory.handover"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: MemoryConvInput = parse_tool_json(input, "memory.handover")?;
        let state = state(&self.slot)?;
        let conv = parsed.conversation_id.clone();
        let text = block_on_async(async move {
            let history = state
                .db
                .get_messages(&conv)
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(|m| buddy_memory::HistoryMessage {
                    role: m.role,
                    content: m.content,
                })
                .collect::<Vec<_>>();
            state.memory.create_handover(&state, &conv, &history).await
        })?;
        Ok(ToolResult { output: text })
    }
}

impl Tool for MemoryMaintainTool {
    fn name(&self) -> &str {
        "memory.maintain"
    }

    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: MemoryConvInput = parse_tool_json(input, "memory.maintain")?;
        let state = state(&self.slot)?;
        let conv = parsed.conversation_id.clone();
        let text = block_on_async(async move { state.memory.run_maintenance(&conv).await })?;
        Ok(ToolResult { output: text })
    }
}

pub const MEMORY_SCHEMAS: &[ToolSchema] = &[
    ToolSchema {
        tool: "memory.handover",
        fields: &[FieldSpec {
            name: "conversation_id",
            label: "conversation",
            required: true,
            memory_keys: &[],
        }],
    },
    ToolSchema {
        tool: "memory.maintain",
        fields: &[FieldSpec {
            name: "conversation_id",
            label: "conversation",
            required: true,
            memory_keys: &[],
        }],
    },
];

pub fn memory_tool_decls() -> &'static [ToolDecl] {
    &[
        ToolDecl {
            name: "memory.handover",
            planner_line: "memory.handover: generate and save a conversation handover. tool_input JSON: {\"conversation_id\": \"...\"}. Use when the user asks for a handover or says /handover.",
        },
        ToolDecl {
            name: "memory.maintain",
            planner_line: "memory.maintain: run memory maintenance (dedup/archive). tool_input JSON: {\"conversation_id\": \"...\"}. Use when the user asks to maintain memory or says /maintain.",
        },
    ]
}
