//! Declarative input schemas for executable tools.
//!
//! Clarification validates Brain plans against these schemas before Core runs.
//! Schemas describe required/optional fields only — they do not plan or execute.

use serde_json::Value;

/// One field on a tool's input object.
#[derive(Debug, Clone, Copy)]
pub struct FieldSpec {
    /// JSON key in `tool_input`.
    pub name: &'static str,
    /// Human-readable label used when asking the user.
    pub label: &'static str,
    pub required: bool,
    /// Preference / memory keys Clarification may consult before asking.
    pub memory_keys: &'static [&'static str],
}

/// Schema for one executable tool.
#[derive(Debug, Clone, Copy)]
pub struct ToolSchema {
    pub tool: &'static str,
    pub fields: &'static [FieldSpec],
}

impl ToolSchema {
    /// Required fields that are missing or empty in `input`.
    pub fn missing_required(&self, input: &Value) -> Vec<&'static FieldSpec> {
        self.fields
            .iter()
            .filter(|f| f.required && !field_present(input, f.name))
            .collect()
    }

    pub fn field(&self, name: &str) -> Option<&'static FieldSpec> {
        self.fields.iter().find(|f| f.name == name)
    }
}

fn field_present(input: &Value, name: &str) -> bool {
    // Batch create: {"events":[{...}, ...]} — require each item to satisfy
    // the same schema keys when validating calendar.create_event.
    if let Some(arr) = input.get("events").and_then(|e| e.as_array()) {
        if !arr.is_empty() {
            return arr.iter().all(|item| field_present(item, name));
        }
    }

    match input.get(name) {
        None => false,
        Some(Value::Null) => false,
        Some(Value::String(s)) => !s.trim().is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
        Some(Value::Number(_)) | Some(Value::Bool(_)) => true,
    }
}
