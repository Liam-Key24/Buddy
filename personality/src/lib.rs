//! Personality controls *how* Buddy communicates.
//!
//! It does not plan, execute tools, or store memory. Clarification decides
//! *what* to ask; this crate only phrases the ask and lightly styles replies.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Configurable communication profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityProfile {
    pub name: String,
    /// friendly | professional | casual
    pub tone: String,
    /// concise | normal | detailed
    pub verbosity: String,
    /// low | medium | high
    pub humour: String,
    /// low | medium | high
    pub confidence: String,
    pub proactive: bool,
    pub uses_analogies: bool,
    pub uses_emojis: bool,
}

impl Default for PersonalityProfile {
    fn default() -> Self {
        Self {
            name: "Buddy".into(),
            tone: "friendly".into(),
            verbosity: "concise".into(),
            humour: "low".into(),
            confidence: "high".into(),
            proactive: true,
            uses_analogies: true,
            uses_emojis: false,
        }
    }
}

impl PersonalityProfile {
    /// Load from a JSON settings string; falls back to defaults on error.
    pub fn from_settings_json(raw: Option<&str>) -> Self {
        raw.and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }

    pub fn to_settings_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }
}

/// Facts Clarification wants asked — no phrasing yet.
#[derive(Debug, Clone)]
pub struct ClarificationAsk {
    pub field_labels: Vec<String>,
    pub context_hint: Option<String>,
}

/// Phrase a clarification question according to the profile.
pub fn phrase_clarification(profile: &PersonalityProfile, ask: &ClarificationAsk) -> String {
    let labels = &ask.field_labels;
    if labels.is_empty() {
        return String::new();
    }

    let body = if labels.len() == 1 {
        single_field_question(profile, &labels[0], ask.context_hint.as_deref())
    } else {
        combined_fields_question(profile, labels)
    };

    finish(profile, body)
}

/// Turn a tool's raw output into a chat-ready reply for passthrough mode.
///
/// Plain strings pass through. JSON becomes a short natural-language summary
/// so users never see `{"deleted":true,...}` in the chat.
pub fn phrase_tool_result(tool: &str, output: &str) -> String {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return "Done.".into();
    }

    match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => phrase_json_result(tool, &value).unwrap_or_else(|| trimmed.to_string()),
        Err(_) => trimmed.to_string(),
    }
}

/// Light styling for Brain/Core replies — never changes meaning or drops content.
pub fn style_response(profile: &PersonalityProfile, content: &str) -> String {
    let mut out = content.trim().to_string();
    if !profile.uses_emojis {
        out = strip_emojis(&out);
    }
    // Verbosity/tone affect phrasing of clarification questions only.
    // Final Brain/Core content is never summarised or truncated here.
    let _ = &profile.verbosity;
    out
}

fn phrase_json_result(tool: &str, value: &Value) -> Option<String> {
    if let Some(msg) = phrase_delete(value) {
        return Some(msg);
    }

    if let Some(msg) = phrase_scheduling(tool, value) {
        return Some(msg);
    }

    if let Some(arr) = value.as_array() {
        if tool.contains("find_free_time") {
            return Some(phrase_free_slots(arr));
        }
        return Some(phrase_list(tool, arr));
    }

    if let Some(obj) = value.as_object() {
        // Conflict-aware create/update: { status: "ok"|"conflict", ... }
        if let Some(status) = obj.get("status").and_then(|v| v.as_str()) {
            if status == "conflict" {
                return Some(phrase_conflict(obj));
            }
            if status == "ok" {
                if let Some(event) = obj.get("event") {
                    if let Some(title) = event.get("title").and_then(|v| v.as_str()) {
                        return Some(format!("Created “{title}”."));
                    }
                }
            }
        }

        if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
            let action = if tool.contains("block_time") {
                "Blocked time for"
            } else if tool.contains("create") || tool.contains("duplicate") {
                "Created"
            } else if tool.contains("update") {
                "Updated"
            } else if tool.contains("dream") && tool.contains("log") {
                "Logged dream"
            } else if tool.starts_with("dream") {
                "Saved dream"
            } else if tool.starts_with("work") {
                "Logged"
            } else {
                "Got"
            };
            let loc = obj
                .get("location")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| format!(" at {s}"))
                .unwrap_or_default();
            let when = phrase_time_range(
                obj.get("start").or_else(|| obj.get("start_time")),
                obj.get("end").or_else(|| obj.get("end_time")),
            );
            if tool.contains("block_time") {
                return Some(format!("{action} “{title}”{when}."));
            }
            return Some(format!("{action} “{title}”{loc}{when}."));
        }

        if tool.contains("stats") || obj.contains_key("total_sales") || obj.contains_key("hours")
        {
            return Some(format_object_summary(obj));
        }
    }

    None
}

fn phrase_scheduling(tool: &str, value: &Value) -> Option<String> {
    let obj = value.as_object()?;

    if tool.contains("get_capacity")
        || (obj.contains_key("free_hours") && obj.contains_key("booked_hours"))
    {
        return Some(phrase_capacity(obj));
    }

    if tool.contains("day_summary") || obj.contains_key("focus_blocks") {
        return Some(phrase_day_summary(obj));
    }

    if tool.contains("schedule_task")
        || (obj.contains_key("scheduled") && obj.contains_key("unscheduled"))
    {
        return Some(phrase_schedule_result(obj));
    }

    if tool.contains("plan_day") || obj.contains_key("proposed") {
        return Some(phrase_plan_day(obj));
    }

    None
}

fn phrase_capacity(obj: &serde_json::Map<String, Value>) -> String {
    let free = num_field(obj, "free_hours");
    let booked = num_field(obj, "booked_hours");
    let meeting = num_field(obj, "meeting_hours");
    let focus = num_field(obj, "focus_hours");
    let waking = num_field(obj, "waking_hours");
    let overloaded = obj
        .get("overloaded")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut msg = format!(
        "Today: {free:.1}h free · {booked:.1}h booked (meetings {meeting:.1}h, focus {focus:.1}h) · {waking:.1}h waking."
    );
    if overloaded {
        msg.push_str(" Day looks overloaded.");
    }
    msg
}

fn phrase_day_summary(obj: &serde_json::Map<String, Value>) -> String {
    let mut parts = Vec::new();
    if let Some(cap) = obj.get("capacity").and_then(|v| v.as_object()) {
        parts.push(phrase_capacity(cap));
    }
    if let Some(suggestions) = obj.get("suggestions").and_then(|v| v.as_array()) {
        let tips: Vec<&str> = suggestions
            .iter()
            .filter_map(|s| s.get("message").and_then(|m| m.as_str()))
            .take(2)
            .collect();
        if !tips.is_empty() {
            parts.push(format!("Suggestions: {}", tips.join(" · ")));
        }
    }
    if parts.is_empty() {
        "Here's your day summary.".into()
    } else {
        parts.join(" ")
    }
}

fn phrase_schedule_result(obj: &serde_json::Map<String, Value>) -> String {
    let scheduled = obj
        .get("scheduled")
        .and_then(|v| v.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);
    let unscheduled = obj
        .get("unscheduled")
        .and_then(|v| v.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    let mut parts = Vec::new();
    if !scheduled.is_empty() {
        let titles = scheduled
            .iter()
            .filter_map(|s| s.get("title").and_then(|t| t.as_str()))
            .map(|t| format!("“{t}”"))
            .collect::<Vec<_>>();
        parts.push(format!(
            "Scheduled {}: {}.",
            if titles.len() == 1 {
                "1 block".into()
            } else {
                format!("{} blocks", titles.len())
            },
            join_natural(&titles)
        ));
    }
    if !unscheduled.is_empty() {
        let titles = unscheduled
            .iter()
            .filter_map(|s| s.get("title").and_then(|t| t.as_str()))
            .map(|t| format!("“{t}”"))
            .collect::<Vec<_>>();
        parts.push(format!(
            "Could not fit {} without violating Work/Sleep/buffers.",
            join_natural(&titles)
        ));
    }
    if let Some(suggestions) = obj.get("suggestions").and_then(|v| v.as_array()) {
        if let Some(msg) = suggestions
            .first()
            .and_then(|s| s.get("message").and_then(|m| m.as_str()))
        {
            parts.push(msg.to_string());
        }
    }
    if parts.is_empty() {
        "No tasks were scheduled.".into()
    } else {
        parts.join(" ")
    }
}

fn phrase_plan_day(obj: &serde_json::Map<String, Value>) -> String {
    let proposed = obj
        .get("proposed")
        .and_then(|v| v.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);
    if proposed.is_empty() {
        let unscheduled = obj
            .get("unscheduled")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        if unscheduled > 0 {
            return format!(
                "Couldn't place {unscheduled} task(s) without overlapping Work, Sleep, or buffers."
            );
        }
        return "No plan blocks proposed for that day.".into();
    }
    let mut items: Vec<&Value> = proposed.iter().collect();
    items.sort_by_key(|p| {
        p.get("start")
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
            .unwrap_or(0)
    });
    let lines: Vec<String> = items
        .iter()
        .filter_map(|p| {
            let title = p.get("title")?.as_str()?;
            let when = phrase_time_range(p.get("start"), p.get("end"));
            Some(format!("“{title}”{when}"))
        })
        .take(6)
        .collect();
    format!("Planned {}: {}.", lines.len(), join_natural(&lines))
}

fn phrase_free_slots(items: &[Value]) -> String {
    if items.is_empty() {
        return "No free slots found that respect Work, Sleep, and buffers.".into();
    }
    let slots: Vec<String> = items
        .iter()
        .take(3)
        .filter_map(|s| {
            let when = phrase_time_range(s.get("start"), s.get("end"));
            if when.is_empty() {
                None
            } else {
                Some(when.trim().trim_start_matches(' ').to_string())
            }
        })
        .collect();
    if slots.is_empty() {
        format!("Found {} free slot(s).", items.len())
    } else if slots.len() == 1 {
        format!("You're free {}.", slots[0].trim_start_matches("from "))
    } else {
        format!("Best free times: {}.", join_natural(&slots))
    }
}

fn phrase_conflict(obj: &serde_json::Map<String, Value>) -> String {
    let report = obj.get("report").and_then(|v| v.as_object()).unwrap_or(obj);
    let msg = report
        .get("conflicts")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("message").and_then(|m| m.as_str()))
        .unwrap_or("That time conflicts with your schedule.");
    let alt = report
        .get("suggestions")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .find(|s| s.get("action").and_then(|a| a.as_str()) == Some("use_slot"))
        .and_then(|s| s.get("message").and_then(|m| m.as_str()));
    match alt {
        Some(a) => format!("{msg} {a}"),
        None => msg.to_string(),
    }
}

fn phrase_time_range(start: Option<&Value>, end: Option<&Value>) -> String {
    let Some(s) = start.and_then(value_as_i64) else {
        return String::new();
    };
    let Some(e) = end.and_then(value_as_i64) else {
        return format!(" from {}", format_local_time(s));
    };
    format!(" from {} to {}", format_local_time(s), format_local_time(e))
}

fn value_as_i64(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_f64().map(|f| f as i64))
        .or_else(|| v.as_str()?.parse().ok())
}

fn format_local_time(ms: i64) -> String {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|dt| dt.format("%I:%M %p").to_string().trim_start_matches('0').to_string())
        .unwrap_or_else(|| ms.to_string())
}

fn num_field(obj: &serde_json::Map<String, Value>, key: &str) -> f64 {
    obj.get(key)
        .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
        .unwrap_or(0.0)
}

fn phrase_delete(value: &Value) -> Option<String> {
    let obj = value.as_object()?;
    if obj.get("deleted").and_then(|v| v.as_bool()) != Some(true) {
        return None;
    }

    let count = obj
        .get("count")
        .and_then(|v| v.as_u64())
        .or_else(|| {
            obj.get("ids")
                .and_then(|v| v.as_array())
                .map(|a| a.len() as u64)
        });

    if obj.get("all").and_then(|v| v.as_bool()) == Some(true) {
        return Some(match count {
            Some(0) => "Your calendar was already empty.".into(),
            Some(1) => "Deleted the only event on your calendar.".into(),
            Some(n) => format!("Deleted all {n} events from your calendar."),
            None => "Deleted all events from your calendar.".into(),
        });
    }

    if let Some(query) = obj.get("query").and_then(|v| v.as_str()) {
        return Some(match count {
            Some(1) => format!("Deleted 1 event matching “{query}”."),
            Some(n) => format!("Deleted {n} events matching “{query}”."),
            None => format!("Deleted events matching “{query}”."),
        });
    }

    if obj.get("id").and_then(|v| v.as_str()).is_some() {
        return Some("Deleted it.".into());
    }

    Some("Deleted.".into())
}

fn phrase_list(tool: &str, items: &[Value]) -> String {
    let noun = if tool.contains("dream") {
        "dream"
    } else if tool.contains("block") {
        "block"
    } else {
        "event"
    };

    if items.is_empty() {
        return format!("No {noun}s found.");
    }

    let titles: Vec<&str> = items
        .iter()
        .filter_map(|item| {
            item.get("title")
                .or_else(|| item.get("name"))
                .and_then(|v| v.as_str())
        })
        .take(5)
        .collect();

    if titles.is_empty() {
        let n = items.len();
        let label = if n == 1 {
            noun.to_string()
        } else {
            format!("{noun}s")
        };
        return format!("Found {n} {label}.");
    }

    let listed = join_natural(
        &titles
            .iter()
            .map(|t| format!("“{t}”"))
            .collect::<Vec<_>>(),
    );
    let extra = items.len().saturating_sub(titles.len());
    if extra > 0 {
        format!(
            "Found {} {noun}s: {listed}, and {extra} more.",
            items.len()
        )
    } else if items.len() == 1 {
        format!("Found 1 {noun}: {listed}.")
    } else {
        format!("Found {} {noun}s: {listed}.", items.len())
    }
}

fn format_object_summary(obj: &serde_json::Map<String, Value>) -> String {
    let parts: Vec<String> = obj
        .iter()
        .filter_map(|(k, v)| match v {
            Value::String(s) if !s.is_empty() => Some(format!("{k}: {s}")),
            Value::Number(n) => Some(format!("{k}: {n}")),
            Value::Bool(b) => Some(format!("{k}: {b}")),
            _ => None,
        })
        .take(6)
        .collect();
    if parts.is_empty() {
        "Done.".into()
    } else {
        parts.join(" · ")
    }
}

fn single_field_question(
    profile: &PersonalityProfile,
    label: &str,
    context: Option<&str>,
) -> String {
    let friendly = profile.tone == "friendly" || profile.tone == "casual";
    match (friendly, label, context) {
        (true, "date and time" | "time" | "finish time" | "start time", Some(ctx)) => {
            format!("What time would you like for {ctx}?")
        }
        (true, "date and time" | "time", _) => "What time works for you?".into(),
        (true, "title" | "event", _) => "What should I call it?".into(),
        (true, "location", _) => "Where should it be?".into(),
        (true, "recipient", _) => "Who should I send it to?".into(),
        (true, "message" | "idea" | "dream description", _) => {
            format!("What would you like the {label} to be?")
        }
        (true, _, Some(ctx)) => format!("What's the {label} for {ctx}?"),
        (true, _, _) => format!("What's the {label}?"),
        (false, _, Some(ctx)) => format!("Please provide the {label} for {ctx}."),
        (false, _, _) => format!("Please provide the {label}."),
    }
}

fn combined_fields_question(profile: &PersonalityProfile, labels: &[String]) -> String {
    let list = join_natural(labels);
    if profile.tone == "friendly" || profile.tone == "casual" {
        format!("Sure — what's the {list}?")
    } else {
        format!("Please provide the {list}.")
    }
}

fn join_natural(labels: &[String]) -> String {
    match labels.len() {
        0 => String::new(),
        1 => labels[0].clone(),
        2 => format!("{} and {}", labels[0], labels[1]),
        _ => {
            let head = labels[..labels.len() - 1].join(", ");
            format!("{}, and {}", head, labels[labels.len() - 1])
        }
    }
}

fn finish(profile: &PersonalityProfile, mut body: String) -> String {
    if !profile.uses_emojis {
        body = strip_emojis(&body);
    }
    body
}

fn strip_emojis(s: &str) -> String {
    s.chars()
        .filter(|c| {
            let u = *c as u32;
            // Keep basic punctuation/letters; drop common emoji blocks.
            !(u >= 0x1F300 && u <= 0x1FAFF)
                && !(u >= 0x2600 && u <= 0x27BF)
                && *c != '\u{FE0F}'
                && *c != '\u{200D}'
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friendly_single_time_question() {
        let p = PersonalityProfile::default();
        let q = phrase_clarification(
            &p,
            &ClarificationAsk {
                field_labels: vec!["time".into()],
                context_hint: Some("meeting Tom".into()),
            },
        );
        assert!(q.contains("time"));
        assert!(q.contains("Tom") || q.contains("meet"));
    }

    #[test]
    fn combines_related_fields() {
        let p = PersonalityProfile::default();
        let q = phrase_clarification(
            &p,
            &ClarificationAsk {
                field_labels: vec!["title".into(), "date and time".into(), "location".into()],
                context_hint: None,
            },
        );
        assert!(q.contains("title"));
        assert!(q.contains("location"));
    }

    #[test]
    fn phrases_delete_all_json() {
        let msg = phrase_tool_result(
            "calendar.delete_event",
            r#"{"all":true,"count":196,"deleted":true}"#,
        );
        assert_eq!(msg, "Deleted all 196 events from your calendar.");
    }

    #[test]
    fn phrases_created_event() {
        let msg = phrase_tool_result(
            "calendar.create_event",
            r#"{"id":"1","title":"Standup","location":"Zoom"}"#,
        );
        assert!(msg.contains("Standup"));
        assert!(msg.contains("Created"));
    }

    #[test]
    fn phrases_capacity() {
        let msg = phrase_tool_result(
            "calendar.get_capacity",
            r#"{"date":"2026-07-22","booked_hours":0.0,"meeting_hours":0.0,"focus_hours":0.0,"free_hours":6.75,"waking_hours":14.75,"overloaded":false}"#,
        );
        assert!(msg.contains("6.8h free") || msg.contains("6.75h free"));
        assert!(msg.contains("waking"));
        assert!(!msg.contains("8 hours of capacity"));
    }

    #[test]
    fn phrases_schedule_task_unscheduled() {
        let msg = phrase_tool_result(
            "calendar.schedule_task",
            r#"{"scheduled":[],"unscheduled":[{"title":"Design report","duration_minutes":120}],"suggestions":[{"action":"redistribute","message":"Could not find a free slot."}]}"#,
        );
        assert!(msg.contains("Design report"));
        assert!(msg.contains("Could not"));
        assert!(!msg.contains("\"scheduled\""));
    }

    #[test]
    fn phrases_block_time() {
        let msg = phrase_tool_result(
            "calendar.block_time",
            r#"{"title":"Coding","start":1784800000000,"end":1784810800000,"flexibility":"flexible","priority":"normal","score":100.0,"reasons":[]}"#,
        );
        assert!(msg.contains("Coding"));
        assert!(msg.contains("Blocked"));
    }

    #[test]
    fn phrases_free_slots() {
        let msg = phrase_tool_result(
            "calendar.find_free_time",
            r#"[{"start":1784810800000,"end":1784818000000,"score":90.0,"reasons":["preferred focus period"]}]"#,
        );
        assert!(msg.contains("free") || msg.contains("Free") || msg.contains("from"));
        assert!(!msg.contains("\"score\""));
    }
}
