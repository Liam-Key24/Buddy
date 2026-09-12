//! Clarification decides whether an execution plan has enough information.
//!
//! It validates Brain output against tool schemas, fills from Memory when
//! confidence is high, and reports what to ask — without planning, executing,
//! or phrasing questions (Personality owns phrasing).

use buddy_core::{AskKind, FieldSpec, ToolSchema};
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

/// One selectable option for a choice ask (owned, serializable).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AskChoice {
    pub id: String,
    pub label: String,
    /// JSON value fragment to merge into tool_input.
    pub value: String,
}

/// A required field the user still needs to supply.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MissingField {
    pub name: String,
    pub label: String,
    pub ask_kind: AskKind,
    #[serde(default)]
    pub choices: Vec<AskChoice>,
}

impl MissingField {
    pub fn from_field_spec(field: &FieldSpec) -> Self {
        Self {
            name: field.name.to_string(),
            label: field.label.to_string(),
            ask_kind: field.ask_kind,
            choices: field
                .choices
                .iter()
                .map(|c| AskChoice {
                    id: c.id.to_string(),
                    label: c.label.to_string(),
                    value: c.value.to_string(),
                })
                .collect(),
        }
    }

    /// Synthetic missing field for a dynamic tool `_ask` follow-up.
    pub fn from_dynamic_ask(
        name: impl Into<String>,
        label: impl Into<String>,
        choices: Vec<AskChoice>,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            ask_kind: AskKind::Choice,
            choices,
        }
    }
}

/// Outcome of validating a plan against a tool schema.
#[derive(Debug, Clone)]
pub enum ClarifyResult {
    /// Ready for Core. `tool_input` may include Memory fills.
    Ready { tool_input: String },
    /// Need the user to supply the listed fields.
    NeedsInput {
        tool_input: String,
        missing: Vec<MissingField>,
        context_hint: Option<String>,
    },
}

/// Phase of a durable open job for one conversation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum JobPhase {
    #[default]
    NeedsInput,
    AwaitingConfirm,
    Running,
}

/// Pending clarification / open job stored between turns (Memory owns persistence).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingClarification {
    pub tool: String,
    pub tool_input: String,
    /// Structured missing fields (preferred).
    #[serde(default)]
    pub missing: Vec<MissingField>,
    /// Legacy label list — kept for older pending state / force sentinel.
    #[serde(default)]
    pub missing_labels: Vec<String>,
    #[serde(default)]
    pub conversation_id: String,
    /// True when this pending came from a tool output `_ask` follow-up.
    #[serde(default)]
    pub follow_up: bool,
    /// JSON agent scratchpad so resolve_clarification can continue the loop.
    #[serde(default)]
    pub agent_scratchpad: Option<String>,
    /// Original user goal for the agent turn / open job.
    #[serde(default)]
    pub agent_goal: Option<String>,
    /// Open-job phase (needs input vs awaiting confirm after propose).
    #[serde(default)]
    pub phase: JobPhase,
    /// Compact last tool proposal JSON (for confirm → apply).
    #[serde(default)]
    pub last_proposal: Option<String>,
}

impl PendingClarification {
    pub fn from_needs(
        tool: impl Into<String>,
        tool_input: impl Into<String>,
        missing: Vec<MissingField>,
        conversation_id: impl Into<String>,
    ) -> Self {
        let labels: Vec<String> = missing.iter().map(|m| m.label.clone()).collect();
        Self {
            tool: tool.into(),
            tool_input: tool_input.into(),
            missing_labels: labels,
            missing,
            conversation_id: conversation_id.into(),
            follow_up: false,
            agent_scratchpad: None,
            agent_goal: None,
            phase: JobPhase::NeedsInput,
            last_proposal: None,
        }
    }

    pub fn labels(&self) -> Vec<String> {
        if !self.missing.is_empty() {
            return self.missing.iter().map(|m| m.label.clone()).collect();
        }
        self.missing_labels.clone()
    }

    pub fn primary_missing(&self) -> Option<&MissingField> {
        self.missing.first()
    }

    pub fn goal(&self) -> &str {
        self.agent_goal.as_deref().unwrap_or("")
    }
}

/// True when the user is confirming a proposed write.
pub fn is_confirm_phrase(text: &str) -> bool {
    let t = text.trim().to_ascii_lowercase();
    matches!(
        t.as_str(),
        "yes"
            | "y"
            | "ok"
            | "okay"
            | "sure"
            | "confirm"
            | "do it"
            | "go ahead"
            | "please"
            | "add it"
            | "add them"
            | "add these"
            | "add to calendar"
            | "put it on my calendar"
            | "put it on the calendar"
            | "schedule it"
            | "book it"
            | "apply"
            | "save it"
    ) || t.starts_with("yes ")
        || t.contains("add to calendar")
        || t.contains("add it to")
        || t.contains("add these to")
        || t.contains("add them to")
        || ((t.contains("add these") || t.contains("add them") || t.contains("add it"))
            && t.contains("calendar"))
        || t == "no thank you add to calendar"
        || (t.contains("add to calendar") || t.contains("add it"))
            && (t.contains("please") || t.contains("thank") || t.contains("yes"))
}

/// Soft scheduling constraint to merge into an open job (not a new intent).
pub fn is_soft_constraint_phrase(text: &str) -> bool {
    let t = text.trim().to_ascii_lowercase();
    t.contains("after work")
        || t.contains("before work")
        || t.contains("morning")
        || t.contains("evening")
        || t.contains("afternoon")
        || t.contains("weekend")
        || t.contains("prefer")
        || t == "you tell me"
        || t == "you decide"
        || t == "decide for me"
        || t == "whatever works"
        || t.starts_with("you tell me")
        || t.starts_with("you decide")
}

pub fn is_cancel_phrase(text: &str) -> bool {
    let t = text.trim().to_ascii_lowercase();
    matches!(
        t.as_str(),
        "cancel" | "nevermind" | "never mind" | "stop" | "forget it" | "no"
    )
}

/// Parse duration like "2 hours", "90 min", "for 2h" → minutes.
pub fn parse_duration_minutes(text: &str) -> Option<u32> {
    let lower = text.to_ascii_lowercase();
    if let Some(c) = regex_first_u32(&lower, r"\b(\d+)\s*(?:hours?|hrs?|h)\b") {
        return Some((c * 60).max(15));
    }
    if let Some(c) = regex_first_u32(&lower, r"\b(\d+)\s*(?:minutes?|mins?|m)\b") {
        return Some(c.max(15));
    }
    None
}

fn regex_first_u32(text: &str, _pat: &str) -> Option<u32> {
    // Avoid regex crate dependency: simple scanners.
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let num: u32 = text[start..i].parse().ok()?;
            let rest = text[i..].trim_start();
            if rest.starts_with("hour")
                || rest.starts_with("hr")
                || rest.starts_with('h')
                    && (rest.len() == 1
                        || rest.as_bytes().get(1).map(|b| !b.is_ascii_alphanumeric())
                            == Some(true))
            {
                return Some(num);
            }
            if rest.starts_with("minute") || rest.starts_with("min") {
                return Some(num);
            }
            continue;
        }
        i += 1;
    }
    None
}

/// Merge free-text into pending tool_input for common schedule fields.
/// Returns updated tool_input JSON.
pub fn merge_free_text_into_tool_input(tool_input: &str, text: &str) -> String {
    let mut root: Value = serde_json::from_str(tool_input).unwrap_or_else(|_| json!({}));
    if !root.is_object() {
        root = json!({});
    }
    let obj = root.as_object_mut().unwrap();

    if let Some(mins) = parse_duration_minutes(text) {
        obj.insert("duration_minutes".into(), json!(mins));
    }

    // Title: "climbing for 2 hours" / bare activity word when title empty.
    let title_empty = obj
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    if title_empty {
        let lower = text.to_ascii_lowercase();
        for (needle, title) in [
            ("climb", "Climbing"),
            ("gym", "Gym"),
            ("yoga", "Yoga"),
            ("tennis", "Tennis"),
            ("run", "Run"),
            ("workout", "Workout"),
        ] {
            if lower.contains(needle) {
                obj.insert("title".into(), json!(title));
                break;
            }
        }
    }

    if is_soft_constraint_phrase(text) {
        let note = text.trim();
        let lower = note.to_ascii_lowercase();
        let prev = obj
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let next = if prev.is_empty() {
            note.to_string()
        } else if prev.contains(note) {
            prev
        } else {
            format!("{prev}; {note}")
        };
        obj.insert("description".into(), json!(next));

        // Structured prefer for the calendar pack (reads lifestyle Work end).
        if lower.contains("after work")
            || lower.contains("evening")
            || (lower.contains("prefer") && lower.contains("evening"))
        {
            obj.insert("prefer_after_work".into(), json!(true));
            let mut constraints = obj
                .get("constraints")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if !constraints.iter().any(|c| c.as_str() == Some("after_work")) {
                constraints.push(json!("after_work"));
            }
            obj.insert("constraints".into(), json!(constraints));
        } else if lower.contains("morning") || lower.contains("before work") {
            obj.insert("prefer_after_work".into(), json!(false));
        }
    }

    fan_out_duration_into_tasks(&mut root);
    root.to_string()
}

/// Set apply flag on tool_input JSON.
pub fn set_apply_flag(tool_input: &str, apply: bool) -> String {
    let mut root: Value = serde_json::from_str(tool_input).unwrap_or_else(|_| json!({}));
    if !root.is_object() {
        root = json!({});
    }
    if let Some(obj) = root.as_object_mut() {
        obj.insert("apply".into(), json!(apply));
        obj.insert(
            "mode".into(),
            json!(if apply { "commit" } else { "propose" }),
        );
    }
    root.to_string()
}

/// Detect propose-only calendar results that need user confirm.
pub fn looks_like_proposal(tool: &str, tool_input: &str, output: &str) -> bool {
    if !tool.starts_with("calendar.") {
        return false;
    }
    let input: Value = serde_json::from_str(tool_input).unwrap_or(Value::Null);
    if input.get("apply").and_then(|v| v.as_bool()) == Some(true) {
        return false;
    }
    let out: Value = serde_json::from_str(output).unwrap_or(Value::Null);
    let has_blocks = out
        .get("scheduled")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
        || out
            .get("proposed")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
    if !has_blocks {
        return false;
    }
    out.get("apply").and_then(|v| v.as_bool()) == Some(false)
        || input.get("apply").and_then(|v| v.as_bool()) == Some(false)
        || input
            .get("mode")
            .and_then(|v| v.as_str())
            .map(|s| s.eq_ignore_ascii_case("propose"))
            .unwrap_or(false)
        || out
            .get("status")
            .and_then(|v| v.as_str())
            .map(|s| s.eq_ignore_ascii_case("proposed"))
            .unwrap_or(false)
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
    fan_out_duration_into_tasks(&mut value);
    infer_safe_defaults(tool, &mut value, config.confidence_threshold);

    let missing_specs = missing_required_fields(tool, schema, &value);
    if missing_specs.is_empty() {
        return ClarifyResult::Ready {
            tool_input: value.to_string(),
        };
    }

    let missing: Vec<MissingField> = missing_specs
        .iter()
        .map(|f| MissingField::from_field_spec(f))
        .collect();
    let context_hint = value
        .get("title")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    ClarifyResult::NeedsInput {
        tool_input: value.to_string(),
        missing,
        context_hint,
    }
}

/// Merge a resolved field value (JSON fragment or raw string) into tool_input JSON.
pub fn merge_field_value(tool_input: &str, field: &str, value_json: &str) -> String {
    let mut root: Value = serde_json::from_str(tool_input).unwrap_or_else(|_| json!({}));
    if !root.is_object() {
        root = json!({});
    }
    let parsed = serde_json::from_str::<Value>(value_json.trim())
        .unwrap_or_else(|_| Value::String(value_json.to_string()));
    if let Some(obj) = root.as_object_mut() {
        obj.insert(field.to_string(), parsed);
    }
    if field == "duration_minutes" {
        fan_out_duration_into_tasks(&mut root);
    }
    root.to_string()
}

/// Copy top-level `duration_minutes` into any `tasks[]` items missing a duration.
fn fan_out_duration_into_tasks(value: &mut Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    let Some(mins) = coerce_u32(obj.get("duration_minutes")) else {
        return;
    };
    // Normalize top-level to a number when it was a numeric string.
    obj.insert("duration_minutes".into(), json!(mins));
    let Some(arr) = obj.get_mut("tasks").and_then(|t| t.as_array_mut()) else {
        return;
    };
    for item in arr {
        let Some(task) = item.as_object_mut() else {
            continue;
        };
        if !field_present(task.get("duration_minutes")) {
            task.insert("duration_minutes".into(), json!(mins));
        }
    }
}

fn coerce_u32(v: Option<&Value>) -> Option<u32> {
    match v? {
        Value::Number(n) => n.as_u64().map(|n| n as u32),
        Value::String(s) => {
            let trimmed = s.trim();
            if let Ok(n) = trimmed.parse::<u32>() {
                return Some(n);
            }
            parse_duration_minutes(trimmed)
        }
        _ => None,
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
                    if field.name == "duration_minutes" {
                        if let Some(mins) = coerce_u32(Some(&Value::String(val.clone()))) {
                            obj.insert(field.name.to_string(), json!(mins));
                            break;
                        }
                        // Non-numeric preference — skip rather than leave a bad string.
                        continue;
                    }
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
    if tool == "calendar.create_event" || tool == "calendar.pin" {
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

    // Scheduling windows: only fill start/end once duration is known.
    if matches!(
        tool,
        "calendar.find_free_time"
            | "calendar.block_time"
            | "calendar.schedule_task"
            | "calendar.organize"
            | "calendar.look"
    ) {
        if let Some(obj) = value.as_object_mut() {
            if field_present(obj.get("duration_minutes")) {
                let now_ms = chrono_like_now_ms();
                if !field_present(obj.get("start")) && !field_present(obj.get("start_time")) {
                    obj.insert("start".into(), json!(now_ms));
                }
                if !field_present(obj.get("end")) && !field_present(obj.get("end_time")) {
                    // Default search window: 7 days.
                    obj.insert("end".into(), json!(now_ms + 7 * 24 * 60 * 60 * 1000));
                }
            }
        }
    }
}

fn chrono_like_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
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

    if tool == "calendar.schedule_task" {
        let has_top_duration = coerce_u32(value.get("duration_minutes")).is_some();
        let tasks_arr = value.get("tasks").and_then(|t| t.as_array());
        let has_tasks = tasks_arr.map(|a| !a.is_empty()).unwrap_or(false);
        let tasks_all_have_duration = tasks_arr
            .map(|a| {
                !a.is_empty()
                    && a.iter()
                        .all(|item| coerce_u32(item.get("duration_minutes")).is_some())
            })
            .unwrap_or(false);
        let has_title = field_present(value.get("title")) || has_tasks;
        let mut missing = vec![];
        if !has_title {
            if let Some(f) = schema.field("title") {
                missing.push(f);
            }
        }
        if !has_top_duration && !tasks_all_have_duration {
            if let Some(f) = schema.field("duration_minutes") {
                missing.push(f);
            }
        }
        return missing;
    }

    if tool == "calendar.plan_day" {
        let tasks_arr = value.get("tasks").and_then(|t| t.as_array());
        let has_tasks = tasks_arr.map(|a| !a.is_empty()).unwrap_or(false);
        if !has_tasks {
            return schema
                .fields
                .iter()
                .filter(|f| f.name == "tasks")
                .collect();
        }
        let tasks_all_have_duration = tasks_arr
            .map(|a| {
                a.iter()
                    .all(|item| coerce_u32(item.get("duration_minutes")).is_some())
            })
            .unwrap_or(false);
        if tasks_all_have_duration {
            return vec![];
        }
        // Tasks present but durations missing — ask Choice, never re-ask tasks text.
        return schema
            .fields
            .iter()
            .filter(|f| f.name == "duration_minutes")
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
    use buddy_core::{AskKind, ChoiceSpec, FieldSpec, DURATION_MINUTES_CHOICES};
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
                ask_kind: AskKind::Text,
                choices: &[],
            },
            FieldSpec {
                name: "start_time",
                label: "date and time",
                required: true,
                memory_keys: &[],
                ask_kind: AskKind::Text,
                choices: &[],
            },
            FieldSpec {
                name: "end_time",
                label: "end time",
                required: true,
                memory_keys: &[],
                ask_kind: AskKind::Text,
                choices: &[],
            },
            FieldSpec {
                name: "location",
                label: "location",
                required: false,
                memory_keys: &["preferred_meeting_location"],
                ask_kind: AskKind::Text,
                choices: &[],
            },
        ],
    };

    const BLOCK_TIME: ToolSchema = ToolSchema {
        tool: "calendar.block_time",
        fields: &[
            FieldSpec {
                name: "title",
                label: "title",
                required: true,
                memory_keys: &[],
                ask_kind: AskKind::Text,
                choices: &[],
            },
            FieldSpec {
                name: "duration_minutes",
                label: "how long",
                required: true,
                memory_keys: &[],
                ask_kind: AskKind::Choice,
                choices: DURATION_MINUTES_CHOICES,
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
            ClarifyResult::NeedsInput { missing, .. } => {
                assert!(missing.iter().any(|m| m.label.contains("time")));
            }
            other => panic!("expected NeedsInput, got {other:?}"),
        }
    }

    #[test]
    fn asks_duration_with_choice_options() {
        let prefs = MapPrefs(HashMap::new());
        let result = clarify(
            "calendar.block_time",
            r#"{"title":"Coding","start":1,"end":2}"#,
            Some(&BLOCK_TIME),
            &prefs,
            &ClarificationConfig::default(),
        );
        match result {
            ClarifyResult::NeedsInput { missing, .. } => {
                assert_eq!(missing.len(), 1);
                assert_eq!(missing[0].name, "duration_minutes");
                assert_eq!(missing[0].ask_kind, AskKind::Choice);
                assert_eq!(missing[0].choices.len(), 4);
            }
            other => panic!("expected NeedsInput, got {other:?}"),
        }
    }

    #[test]
    fn merge_field_value_inserts_number() {
        let out = merge_field_value("{}", "duration_minutes", "60");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["duration_minutes"], 60);
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

    #[test]
    #[allow(dead_code)]
    fn choice_spec_compile_check() {
        let _: &[ChoiceSpec] = DURATION_MINUTES_CHOICES;
    }

    const SCHEDULE_TASK: ToolSchema = ToolSchema {
        tool: "calendar.schedule_task",
        fields: &[
            FieldSpec {
                name: "title",
                label: "title",
                required: true,
                memory_keys: &[],
                ask_kind: AskKind::Text,
                choices: &[],
            },
            FieldSpec {
                name: "duration_minutes",
                label: "how long",
                required: true,
                memory_keys: &["preferred_activity_duration"],
                ask_kind: AskKind::Choice,
                choices: DURATION_MINUTES_CHOICES,
            },
        ],
    };

    #[test]
    fn schedule_task_with_title_asks_only_duration() {
        let prefs = MapPrefs(HashMap::new());
        let result = clarify(
            "calendar.schedule_task",
            r#"{"title":"Climbing","apply":false}"#,
            Some(&SCHEDULE_TASK),
            &prefs,
            &ClarificationConfig::default(),
        );
        match result {
            ClarifyResult::NeedsInput { missing, tool_input, .. } => {
                assert_eq!(missing.len(), 1);
                assert_eq!(missing[0].name, "duration_minutes");
                let v: Value = serde_json::from_str(&tool_input).unwrap();
                assert_eq!(v["title"], "Climbing");
            }
            other => panic!("expected NeedsInput, got {other:?}"),
        }
    }

    #[test]
    fn schedule_task_tasks_with_durations_ready() {
        let prefs = MapPrefs(HashMap::new());
        let result = clarify(
            "calendar.schedule_task",
            r#"{"tasks":[{"title":"Study","duration_minutes":30,"count":3},{"title":"Climbing","duration_minutes":90,"count":2}],"apply":false,"prefer_after_work":true}"#,
            Some(&SCHEDULE_TASK),
            &prefs,
            &ClarificationConfig::default(),
        );
        match result {
            ClarifyResult::Ready { .. } => {}
            other => panic!("expected Ready for multi-task week plan, got {other:?}"),
        }
    }

    #[test]
    fn confirm_phrases_detected() {
        assert!(is_confirm_phrase("add to calendar"));
        assert!(is_confirm_phrase("yes"));
        assert!(is_confirm_phrase("go ahead"));
        assert!(is_confirm_phrase("add these to my calendar"));
        assert!(is_confirm_phrase("add them to my calendar"));
        assert!(is_confirm_phrase("add these"));
        assert!(!is_confirm_phrase("after work"));
        assert!(!is_confirm_phrase("cancel"));
        assert!(is_soft_constraint_phrase("after work"));
        assert!(is_soft_constraint_phrase("prefer mornings"));
        assert!(is_soft_constraint_phrase("you tell me"));
        assert!(is_soft_constraint_phrase("you decide"));
    }

    #[test]
    fn cancel_phrases_detected() {
        assert!(is_cancel_phrase("cancel"));
        assert!(is_cancel_phrase("nevermind"));
        assert!(is_cancel_phrase("never mind"));
        assert!(is_cancel_phrase("forget it"));
        assert!(is_cancel_phrase("no"));
        assert!(!is_cancel_phrase("yes"));
        assert!(!is_cancel_phrase("go ahead"));
        assert!(!is_cancel_phrase("after work"));
    }

    #[test]
    fn resume_does_not_reask_supplied_title() {
        let prefs = MapPrefs(HashMap::new());
        let first = clarify(
            "calendar.create_event",
            r#"{"title":"Meet Tom"}"#,
            Some(&CREATE),
            &prefs,
            &ClarificationConfig::default(),
        );
        match first {
            ClarifyResult::NeedsInput { missing, .. } => {
                assert!(missing.iter().all(|m| m.name != "title"));
                assert!(missing.iter().any(|m| m.label.contains("time")));
            }
            other => panic!("expected NeedsInput, got {other:?}"),
        }
        let second = clarify(
            "calendar.create_event",
            r#"{"title":"Meet Tom","start_time":1,"end_time":2}"#,
            Some(&CREATE),
            &prefs,
            &ClarificationConfig::default(),
        );
        assert!(
            matches!(second, ClarifyResult::Ready { .. }),
            "supplied fields must resume without a second ask"
        );
    }

    #[test]
    fn merge_duration_from_free_text() {
        let out = merge_free_text_into_tool_input(
            r#"{"title":"Climbing","apply":false}"#,
            "2 hours",
        );
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["duration_minutes"], 120);
        assert_eq!(v["title"], "Climbing");
    }

    #[test]
    fn merge_after_work_sets_prefer_flag() {
        let out = merge_free_text_into_tool_input(
            r#"{"title":"Climbing","duration_minutes":120,"apply":false}"#,
            "after work",
        );
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["prefer_after_work"], true);
        assert!(v["description"].as_str().unwrap().contains("after work"));
    }

    #[test]
    fn set_apply_and_looks_like_proposal() {
        let applied = set_apply_flag(r#"{"title":"Climbing","duration_minutes":120}"#, true);
        let v: Value = serde_json::from_str(&applied).unwrap();
        assert_eq!(v["apply"], true);
        assert!(looks_like_proposal(
            "calendar.schedule_task",
            r#"{"apply":false}"#,
            r#"{"scheduled":[{"title":"Climbing"}],"apply":false}"#,
        ));
        assert!(looks_like_proposal(
            "calendar.organize",
            r#"{"mode":"propose"}"#,
            r#"{"status":"proposed","scheduled":[{"title":"Climbing"}],"apply":false}"#,
        ));
        assert!(!looks_like_proposal(
            "calendar.schedule_task",
            r#"{"apply":true}"#,
            r#"{"scheduled":[{"title":"Climbing"}],"apply":true}"#,
        ));
        assert!(!looks_like_proposal(
            "calendar.create_event",
            "{}",
            r#"{"id":"1"}"#,
        ));
    }

    const PLAN_DAY: ToolSchema = ToolSchema {
        tool: "calendar.plan_day",
        fields: &[
            FieldSpec {
                name: "tasks",
                label: "tasks",
                required: true,
                memory_keys: &[],
                ask_kind: AskKind::Text,
                choices: &[],
            },
            FieldSpec {
                name: "duration_minutes",
                label: "how long",
                required: false,
                memory_keys: &["preferred_activity_duration"],
                ask_kind: AskKind::Choice,
                choices: DURATION_MINUTES_CHOICES,
            },
        ],
    };

    #[test]
    fn plan_day_tasks_without_duration_asks_choice() {
        let prefs = MapPrefs(HashMap::new());
        let result = clarify(
            "calendar.plan_day",
            r#"{"tasks":[{"title":"Gym"},{"title":"Bath"}],"apply":false}"#,
            Some(&PLAN_DAY),
            &prefs,
            &ClarificationConfig::default(),
        );
        match result {
            ClarifyResult::NeedsInput { missing, .. } => {
                assert_eq!(missing.len(), 1);
                assert_eq!(missing[0].name, "duration_minutes");
                assert_eq!(missing[0].ask_kind, AskKind::Choice);
            }
            other => panic!("expected NeedsInput duration, got {other:?}"),
        }
    }

    #[test]
    fn duration_merge_fans_out_into_tasks() {
        let out = merge_field_value(
            r#"{"tasks":[{"title":"Gym"},{"title":"Bath"}],"apply":false}"#,
            "duration_minutes",
            "60",
        );
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["duration_minutes"], 60);
        assert_eq!(v["tasks"][0]["duration_minutes"], 60);
        assert_eq!(v["tasks"][1]["duration_minutes"], 60);
    }

    #[test]
    fn memory_duration_string_becomes_number() {
        let mut map = HashMap::new();
        map.insert(
            "preferred_activity_duration".into(),
            ("120".into(), 0.95),
        );
        let prefs = MapPrefs(map);
        let result = clarify(
            "calendar.schedule_task",
            r#"{"title":"Climbing","apply":false}"#,
            Some(&SCHEDULE_TASK),
            &prefs,
            &ClarificationConfig::default(),
        );
        match result {
            ClarifyResult::Ready { tool_input } => {
                let v: Value = serde_json::from_str(&tool_input).unwrap();
                assert_eq!(v["duration_minutes"], 120);
                assert!(v["duration_minutes"].is_number());
            }
            other => panic!("expected Ready with numeric duration, got {other:?}"),
        }
    }

    #[test]
    fn plan_day_top_duration_fans_out_and_ready() {
        let prefs = MapPrefs(HashMap::new());
        let result = clarify(
            "calendar.plan_day",
            r#"{"tasks":[{"title":"Gym"},{"title":"Bath"}],"duration_minutes":45,"apply":false}"#,
            Some(&PLAN_DAY),
            &prefs,
            &ClarificationConfig::default(),
        );
        match result {
            ClarifyResult::Ready { tool_input } => {
                let v: Value = serde_json::from_str(&tool_input).unwrap();
                assert_eq!(v["tasks"][0]["duration_minutes"], 45);
                assert_eq!(v["tasks"][1]["duration_minutes"], 45);
            }
            other => panic!("expected Ready after fan-out, got {other:?}"),
        }
    }
}
