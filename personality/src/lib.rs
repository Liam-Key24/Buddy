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

    if let Some(arr) = value.as_array() {
        return Some(phrase_list(tool, arr));
    }

    if let Some(obj) = value.as_object() {
        if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
            let action = if tool.contains("create") || tool.contains("duplicate") {
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
            return Some(format!("{action} “{title}”{loc}."));
        }

        if tool.contains("stats") || obj.contains_key("total_sales") || obj.contains_key("hours") {
            return Some(format_object_summary(obj));
        }
    }

    None
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
    fn leaves_plain_strings_alone() {
        let msg = phrase_tool_result("echo", "hello there");
        assert_eq!(msg, "hello there");
    }
}
