//! Generic session context merged into every tool_input before Clarification/Core.
//! Buddy never inspects tool names — tools read the keys they need.

use serde_json::{json, Value};

/// Workspace/session identity available to every executable tool.
#[derive(Debug, Clone)]
pub struct SessionContext {
    pub conversation_id: String,
    pub workspace_path: Option<String>,
    /// Raw user message for this turn (tools may use as fallback prompt/body).
    pub user_message: Option<String>,
}

/// Merge session fields into `tool_input` JSON **only where keys are absent**.
/// Never overwrites Brain/Clarification values. No tool-name branching.
pub fn merge_session_into_input(tool_input: &str, session: &SessionContext) -> String {
    let mut value: Value = serde_json::from_str(tool_input).unwrap_or_else(|_| json!({}));
    if !value.is_object() {
        // Non-object inputs (e.g. echo plain text) are left untouched.
        return tool_input.to_string();
    }
    let obj = value.as_object_mut().unwrap();

    insert_if_absent(obj, "conversation_id", &session.conversation_id);
    // Common provenance alias used by some tools — still generic, not tool-named.
    insert_if_absent(obj, "source_conversation_id", &session.conversation_id);
    if let Some(ws) = &session.workspace_path {
        insert_if_absent(obj, "workspace_path", ws);
    }
    if let Some(msg) = &session.user_message {
        insert_if_absent(obj, "user_message", msg);
    }

    value.to_string()
}

fn insert_if_absent(
    obj: &mut serde_json::Map<String, Value>,
    key: &str,
    value: &str,
) {
    let empty = match obj.get(key) {
        None => true,
        Some(Value::Null) => true,
        Some(Value::String(s)) => s.trim().is_empty(),
        _ => false,
    };
    if empty {
        obj.insert(key.to_string(), Value::String(value.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_missing_keys_only() {
        let session = SessionContext {
            conversation_id: "c1".into(),
            workspace_path: Some("/tmp".into()),
            user_message: Some("hello".into()),
        };
        let out = merge_session_into_input(r#"{"title":"Meet"}"#, &session);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["title"], "Meet");
        assert_eq!(v["conversation_id"], "c1");
        assert_eq!(v["user_message"], "hello");
    }

    #[test]
    fn does_not_overwrite_brain_values() {
        let session = SessionContext {
            conversation_id: "c1".into(),
            workspace_path: None,
            user_message: Some("fallback".into()),
        };
        let out = merge_session_into_input(
            r#"{"conversation_id":"keep","prompt":"from brain"}"#,
            &session,
        );
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["conversation_id"], "keep");
        assert_eq!(v["prompt"], "from brain");
    }
}
