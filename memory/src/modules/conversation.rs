use std::sync::Arc;

use buddy_database::Database;

use crate::error::MemoryError;
use crate::memory_trait::Memory;
use crate::types::{
    MemoryContext, MemoryKind, MemoryRecord, RetrieveQuery, DEFAULT_CONVERSATION_WINDOW,
};

pub struct ConversationMemory {
    db: Arc<Database>,
    window: usize,
}

impl ConversationMemory {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            window: DEFAULT_CONVERSATION_WINDOW,
        }
    }
}

impl Memory for ConversationMemory {
    fn kind(&self) -> MemoryKind {
        MemoryKind::Conversation
    }

    fn save(&self, ctx: &MemoryContext, record: MemoryRecord) -> Result<String, MemoryError> {
        let conversation_id = ctx
            .conversation_id
            .as_deref()
            .ok_or_else(|| MemoryError::Invalid("conversation_id required".into()))?;
        let role = record
            .payload
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("user");
        let content = record
            .payload
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let msg = self.db.add_message(conversation_id, role, content)?;
        Ok(msg.id)
    }

    fn retrieve(&self, query: &RetrieveQuery) -> Result<Vec<MemoryRecord>, MemoryError> {
        let conversation_id = query
            .conversation_id
            .as_deref()
            .ok_or_else(|| MemoryError::Invalid("conversation_id required".into()))?;
        let messages = self.db.get_messages(conversation_id)?;
        let limit = query.limit.unwrap_or(self.window);
        let start = messages.len().saturating_sub(limit);
        Ok(messages[start..]
            .iter()
            .map(|m| MemoryRecord {
                id: Some(m.id.clone()),
                kind: MemoryKind::Conversation,
                payload: serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                    "conversation_id": m.conversation_id,
                }),
                created_at: Some(m.created_at),
                updated_at: Some(m.created_at),
            })
            .collect())
    }

    fn update(&self, _id: &str, _record: MemoryRecord) -> Result<(), MemoryError> {
        Ok(())
    }

    fn delete(&self, _id: &str) -> Result<(), MemoryError> {
        Ok(())
    }

    fn summarize(&self, query: &RetrieveQuery) -> Result<String, MemoryError> {
        let records = self.retrieve(query)?;
        if records.is_empty() {
            return Ok(String::new());
        }
        let lines: Vec<String> = records
            .iter()
            .filter_map(|r| {
                let role = r.payload.get("role")?.as_str()?;
                let content = r.payload.get("content")?.as_str()?;
                Some(format!("{role}: {content}"))
            })
            .collect();
        Ok(lines.join("\n"))
    }
}
