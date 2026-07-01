use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Started,
    Updated,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferenceDetected {
    pub key: String,
    pub value: String,
    pub confidence: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionDetected {
    pub decision: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryEvent {
    MessageAdded {
        role: String,
        content: String,
    },
    TaskStarted {
        objective: String,
        plan: Option<String>,
        files: Vec<String>,
    },
    TaskUpdated {
        objective: Option<String>,
        plan: Option<String>,
        files: Option<Vec<String>>,
        notes: Option<String>,
    },
    TaskCompleted {
        outcome: String,
    },
    ToolExecuted {
        tool: String,
        params: String,
        result: String,
        duration_ms: u64,
        success: bool,
    },
    ToolFailed {
        error: String,
        cause: String,
        resolution: Option<String>,
    },
    DecisionRecorded {
        decision: String,
        reason: String,
    },
    PreferenceDetected {
        key: String,
        value: String,
        confidence: f64,
        source: String,
    },
    ProjectChanged {
        hint: String,
    },
    HandoverRequested,
    SessionEnding,
    ContextLimitApproaching {
        estimated_tokens: usize,
    },
    HandoverSaved {
        summary: serde_json::Value,
    },
    ReflectionSaved {
        attempted: String,
        successful: bool,
        improvements: String,
        lessons: String,
    },
    ProjectSaved {
        section: String,
        content: String,
    },
    ConversationDeleted {
        title: String,
        conversation_id: String,
    },
    ConversationArchivedSaved {
        title: String,
        conversation_id: String,
        summary: String,
        topics: Vec<String>,
        key_facts: Vec<String>,
        decisions: Vec<String>,
    },
}

impl MemoryEvent {
    pub fn needs_extraction(&self) -> bool {
        matches!(
            self,
            MemoryEvent::TaskCompleted { .. }
                | MemoryEvent::ProjectChanged { .. }
                | MemoryEvent::HandoverRequested
                | MemoryEvent::SessionEnding
                | MemoryEvent::ContextLimitApproaching { .. }
                | MemoryEvent::ConversationDeleted { .. }
        )
    }
}
