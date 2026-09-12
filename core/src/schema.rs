//! Declarative input schemas for executable tools.
//!
//! Clarification validates Brain plans against these schemas before Core runs.
//! Schemas describe required/optional fields only — they do not plan or execute.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How Clarification should ask for a missing field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AskKind {
    #[default]
    Text,
    Choice,
}

/// One selectable option for [`AskKind::Choice`] fields.
#[derive(Debug, Clone, Copy)]
pub struct ChoiceSpec {
    /// Short id shown as A/B/C/D in the UI (`"a"`, `"b"`, …).
    pub id: &'static str,
    pub label: &'static str,
    /// JSON value fragment to merge into tool_input (e.g. `"60"` or `"\"home\""`).
    pub value: &'static str,
}

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
    pub ask_kind: AskKind,
    pub choices: &'static [ChoiceSpec],
}

/// Common duration presets (30 / 60 / 90 / 120 minutes) for scheduling tools.
pub const DURATION_MINUTES_CHOICES: &[ChoiceSpec] = &[
    ChoiceSpec {
        id: "a",
        label: "30 minutes",
        value: "30",
    },
    ChoiceSpec {
        id: "b",
        label: "1 hour",
        value: "60",
    },
    ChoiceSpec {
        id: "c",
        label: "90 minutes",
        value: "90",
    },
    ChoiceSpec {
        id: "d",
        label: "2 hours",
        value: "120",
    },
];

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
