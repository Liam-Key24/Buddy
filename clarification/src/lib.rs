//! Clarification decides whether an execution plan has enough information.
//!
//! It validates Brain output against tool schemas, fills from Memory when
//! confidence is high, and reports what to ask — without planning, executing,
//! or phrasing questions (Personality owns phrasing).

use buddy_core::{FieldSpec, ToolSchema};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Lookup known preference values by key (settings or preference memory).
pub trait PreferenceLookup {
    fn get(&self, key: &str) -> Option<(String, f64)>;
}

/// Clarification settings.
#[derive(Debug, Clone)]
pub struct ClarificationConfig {
    /// Auto-fill when inferred confidence is at or above this (0.0–1.0).
    pub confidence_threshold: f64,
}

impl Default for ClarificationConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.75,
        }
    }
}

/// Outcome of validating a plan against a tool schema.
#[derive(Debug, Clone)]
pub enum ClarifyResult {
    /// Ready for Core. `tool_input` may include Memory fills.
    Ready { tool_input: String },
    /// Need the user to supply the listed fields (labels for Personality).
    NeedsInput {
        tool_input: String,
        missing_labels: Vec<String>,
        context_hint: Option<String>,
    },
}

/// Pending clarification stored between turns (Memory owns persistence).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingClarification {
    pub tool: String,
    pub tool_input: String,
    pub missing_labels: Vec<String>,
    #[serde(default)]
    pub conversation_id: String,
}

/// Validate `tool_input` for `tool` against `schema` (if any).
///
/// Tools without a schema are treated as ready (pass-through).
pub fn clarify(
    tool: &str,
    tool_input: &str,
    schema: Option<&ToolSchema>,
    prefs: &dyn PreferenceLookup,
    config: &ClarificationConfig,
) -> ClarifyResult {
    let Some(schema) = schema else {
        return ClarifyResult::Ready {
            tool_input: tool_input.to_string(),
        };
    };

    let mut value: Value = serde_json::from_str(tool_input).unwrap_or_else(|_| json!({}));
    if !value.is_object() && value.get("events").is_none() {
        value = json!({});
    }

    fill_from_memory(&mut value, schema, prefs, config.confidence_threshold);
    infer_safe_defaults(tool, &mut value, config.confidence_threshold);

    let missing = missing_required_fields(tool, schema, &value);
    if missing.is_empty() {
        return ClarifyResult::Ready {
            tool_input: value.to_string(),
        };
    }

    let labels: Vec<String> = missing.iter().map(|f| f.label.to_string()).collect();
    let context_hint = value
        .get("title")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    ClarifyResult::NeedsInput {
        tool_input: value.to_string(),
        missing_labels: labels,
        context_hint,
    }
}

fn fill_from_memory(
    value: &mut Value,
    schema: &ToolSchema,
    prefs: &dyn PreferenceLookup,
    threshold: f64,
) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    for field in schema.fields {
        if field_present(obj.get(field.name)) {
            continue;
        }
        for key in field.memory_keys {
            if let Some((val, confidence)) = prefs.get(key) {
                if confidence >= threshold && !val.trim().is_empty() {
                    obj.insert(field.name.to_string(), Value::String(val));
                    break;
                }
            }
        }
    }
}

/// High-confidence structural defaults (not domain planning).
fn infer_safe_defaults(tool: &str, value: &mut Value, threshold: f64) {
    if threshold > 0.99 {
        return;
    }
    if tool == "calendar.create_event" {
        if let Some(obj) = value.as_object_mut() {
            let start = obj.get("start_time").cloned();
            let end_missing = !field_present(obj.get("end_time"));
            if end_missing {
                if let Some(Value::Number(n)) = start {
                    if let Some(ms) = n.as_i64() {
                        // Default 1 hour — confidence treated as high for duration.
                        obj.insert("end_time".into(), json!(ms + 3_600_000));
                    }
                }
            }
        }
        // Batch events
        if let Some(arr) = value.get_mut("events").and_then(|e| e.as_array_mut()) {
            for item in arr {
                infer_safe_defaults("calendar.create_event", item, threshold);
            }
        }
    }
}

fn missing_required_fields<'a>(
    tool: &str,
    schema: &'a ToolSchema,
    value: &Value,
) -> Vec<&'a FieldSpec> {
    // OR-groups for tools where any one of several keys satisfies the need.
    if tool == "calendar.delete_event" {
        let has_id = field_present(value.get("id"));
        let has_query = field_present(value.get("query"));
        let has_all = value.get("all").and_then(|v| v.as_bool()) == Some(true);
        if has_id || has_query || has_all {
            return vec![];
        }
        return schema
            .fields
            .iter()
            .filter(|f| f.name == "query")
            .collect();
    }

    if tool == "work.set_hours" {
        let has_end = field_present(value.get("end_hm"))
            || field_present(value.get("actual_end_ms"))
            || field_present(value.get("start_hm"))
            || field_present(value.get("actual_start_ms"));
        if has_end {
            return vec![];
        }
        return schema
            .fields
            .iter()
            .filter(|f| f.name == "end_hm")
            .collect();
    }

    schema.missing_required(value)
}

fn field_present(v: Option<&Value>) -> bool {
    match v {
        None => false,
        Some(Value::Null) => false,
        Some(Value::String(s)) => !s.trim().is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
        Some(Value::Number(_)) | Some(Value::Bool(_)) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buddy_core::FieldSpec;
    use std::collections::HashMap;

    struct MapPrefs(HashMap<String, (String, f64)>);
    impl PreferenceLookup for MapPrefs {
        fn get(&self, key: &str) -> Option<(String, f64)> {
            self.0.get(key).cloned()
        }
    }

    const CREATE: ToolSchema = ToolSchema {
        tool: "calendar.create_event",
        fields: &[
            FieldSpec {
                name: "title",
                label: "title",
                required: true,
                memory_keys: &[],
            },
            FieldSpec {
                name: "start_time",
                label: "date and time",
                required: true,
                memory_keys: &[],
            },
            FieldSpec {
                name: "end_time",
                label: "end time",
                required: true,
                memory_keys: &[],
            },
            FieldSpec {
                name: "location",
                label: "location",
                required: false,
                memory_keys: &["preferred_meeting_location"],
            },
        ],
    };

    #[test]
    fn ready_when_required_present() {
        let prefs = MapPrefs(HashMap::new());
        let result = clarify(
            "calendar.create_event",
            r#"{"title":"Lunch","start_time":1,"end_time":2}"#,
            Some(&CREATE),
            &prefs,
            &ClarificationConfig::default(),
        );
        assert!(matches!(result, ClarifyResult::Ready { .. }));
    }

    #[test]
    fn asks_for_missing_time() {
        let prefs = MapPrefs(HashMap::new());
        let result = clarify(
            "calendar.create_event",
            r#"{"title":"Lunch with Sarah"}"#,
            Some(&CREATE),
            &prefs,
            &ClarificationConfig::default(),
        );
        match result {
            ClarifyResult::NeedsInput { missing_labels, .. } => {
                assert!(missing_labels.iter().any(|l| l.contains("time")));
            }
            other => panic!("expected NeedsInput, got {other:?}"),
        }
    }

    /// Complete tool flow (plan → clarify missing → fill → ready for Core).
    #[test]
    fn progressive_flow_ready_after_times_filled() {
        let prefs = MapPrefs(HashMap::new());
        let first = clarify(
            "calendar.create_event",
            r#"{"title":"Meet Tom"}"#,
            Some(&CREATE),
            &prefs,
            &ClarificationConfig::default(),
        );
        assert!(matches!(first, ClarifyResult::NeedsInput { .. }));

        let second = clarify(
            "calendar.create_event",
            r#"{"title":"Meet Tom","start_time":1,"end_time":2}"#,
            Some(&CREATE),
            &prefs,
            &ClarificationConfig::default(),
        );
        assert!(matches!(second, ClarifyResult::Ready { .. }));
    }

    #[test]
    fn unknown_tool_passes_through() {
        let prefs = MapPrefs(HashMap::new());
        let result = clarify(
            "unknown.tool",
            r#"{"anything":true}"#,
            None,
            &prefs,
            &ClarificationConfig::default(),
        );
        assert!(matches!(result, ClarifyResult::Ready { .. }));
    }

    #[test]
    fn fills_location_from_memory() {
        let mut map = HashMap::new();
        map.insert(
            "preferred_meeting_location".into(),
            ("Office".into(), 0.9),
        );
        let prefs = MapPrefs(map);
        let result = clarify(
            "calendar.create_event",
            r#"{"title":"Sync","start_time":1,"end_time":2}"#,
            Some(&CREATE),
            &prefs,
            &ClarificationConfig::default(),
        );
        match result {
            ClarifyResult::Ready { tool_input } => {
                let v: Value = serde_json::from_str(&tool_input).unwrap();
                assert_eq!(v["location"], "Office");
            }
            other => panic!("expected Ready, got {other:?}"),
        }
    }
}
