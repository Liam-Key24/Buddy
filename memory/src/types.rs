use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Conversation,
    Working,
    Project,
    Preference,
    Handover,
    Decision,
    Error,
    Tool,
    Reflection,
}

impl MemoryKind {
    pub fn table_name(&self) -> &'static str {
        match self {
            MemoryKind::Conversation => "messages",
            MemoryKind::Working => "memory_working",
            MemoryKind::Project => "memory_project",
            MemoryKind::Preference => "memory_preference",
            MemoryKind::Handover => "memory_handover",
            MemoryKind::Decision => "memory_decision",
            MemoryKind::Error => "memory_error",
            MemoryKind::Tool => "memory_tool",
            MemoryKind::Reflection => "memory_reflection",
        }
    }

    pub fn retrieval_priority(&self) -> u8 {
        match self {
            MemoryKind::Handover => 0,
            MemoryKind::Working => 1,
            MemoryKind::Conversation => 2,
            MemoryKind::Project => 3,
            MemoryKind::Preference => 4,
            MemoryKind::Decision => 5,
            MemoryKind::Error => 6,
            MemoryKind::Tool => 7,
            MemoryKind::Reflection => 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryContext {
    pub workspace_path: PathBuf,
    pub conversation_id: Option<String>,
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: Option<String>,
    pub kind: MemoryKind,
    pub payload: serde_json::Value,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveQuery {
    pub workspace_path: PathBuf,
    pub conversation_id: Option<String>,
    pub task_id: Option<String>,
    pub keywords: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSection {
    pub kind: MemoryKind,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedContext {
    pub handover: Option<String>,
    pub sections: Vec<ContextSection>,
    pub conversation_messages: Vec<HistoryMessage>,
    pub estimated_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryContextPayload {
    pub handover: Option<String>,
    pub working: Option<String>,
    pub project: Option<String>,
    pub preferences: Option<String>,
    pub decisions: Option<String>,
    pub errors: Option<String>,
    pub tools: Option<String>,
    pub reflections: Option<String>,
}

impl From<&MergedContext> for MemoryContextPayload {
    fn from(ctx: &MergedContext) -> Self {
        let mut payload = MemoryContextPayload {
            handover: ctx.handover.clone(),
            working: None,
            project: None,
            preferences: None,
            decisions: None,
            errors: None,
            tools: None,
            reflections: None,
        };
        for section in &ctx.sections {
            match section.kind {
                MemoryKind::Working => payload.working = Some(section.content.clone()),
                MemoryKind::Project => payload.project = Some(section.content.clone()),
                MemoryKind::Preference => payload.preferences = Some(section.content.clone()),
                MemoryKind::Decision => payload.decisions = Some(section.content.clone()),
                MemoryKind::Error => payload.errors = Some(section.content.clone()),
                MemoryKind::Tool => payload.tools = Some(section.content.clone()),
                MemoryKind::Reflection => payload.reflections = Some(section.content.clone()),
                _ => {}
            }
        }
        payload
    }
}

pub const DEFAULT_CONVERSATION_WINDOW: usize = 50;
pub const DEFAULT_TOOL_WINDOW: usize = 200;
pub const DEFAULT_TOKEN_BUDGET: usize = 4096;
pub const CONTEXT_LIMIT_THRESHOLD: usize = 6000;

pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

pub fn chrono_now() -> i64 {
    buddy_database::chrono_now()
}
