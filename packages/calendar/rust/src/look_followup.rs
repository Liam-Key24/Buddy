//! Compact last-look snapshot so follow-ups like "when was it?" skip the model.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastLookEvent {
    pub id: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
    #[serde(default)]
    pub all_day: bool,
    #[serde(default)]
    pub location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastLookSlot {
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastLookBlock {
    pub kind: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LastLook {
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub focus: String,
    #[serde(default)]
    pub events: Vec<LastLookEvent>,
    #[serde(default)]
    pub slots: Vec<LastLookSlot>,
    #[serde(default)]
    pub schedule: Vec<LastLookBlock>,
    #[serde(default)]
    pub off_days: Vec<String>,
}

pub fn last_look_from_tool_output(output: &str) -> Option<LastLook> {
    let value: Value = serde_json::from_str(output).ok()?;
    let obj = value.as_object()?;
    let focus = obj
        .get("focus")
        .and_then(|v| v.as_str())
        .unwrap_or("events")
        .to_string();
    let when = obj
        .get("when")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let events: Vec<LastLookEvent> = obj
        .get("events")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(parse_event).collect())
        .unwrap_or_default();
    let slots: Vec<LastLookSlot> = obj
        .get("slots")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(parse_slot).collect())
        .unwrap_or_default();
    let schedule: Vec<LastLookBlock> = obj
        .get("schedule")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(parse_block).collect())
        .unwrap_or_default();
    let off_days: Vec<String> = obj
        .get("off_days")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    if events.is_empty()
        && slots.is_empty()
        && schedule.is_empty()
        && focus != "events"
        && focus != "free"
    {
        return None;
    }
    Some(LastLook {
        when,
        focus,
        events,
        slots,
        schedule,
        off_days,
    })
}

pub fn select_followup_payload(look: &LastLook, text: &str) -> Option<Value> {
    let lower = text.trim().to_ascii_lowercase();
    if looks_like_fresh_look(&lower) {
        return None;
    }
    if look.events.is_empty() && look.slots.is_empty() && look.schedule.is_empty() {
        return None;
    }

    if is_working_query(&lower) && !look.schedule.is_empty() {
        return Some(schedule_followup_json(look));
    }

    if let Some(event) = match_title(look, &lower) {
        if is_when_ish(&lower) {
            return Some(json!({
                "focus": "events",
                "events": [event_json(event)],
                "schedule": look.schedule.iter().map(block_json).collect::<Vec<_>>(),
            }));
        }
    }

    if !is_when_followup(&lower) {
        return None;
    }

    if look.focus == "free" && !look.slots.is_empty() {
        return Some(json!({
            "focus": "free",
            "slots": look.slots.iter().map(|s| json!({"start": s.start, "end": s.end})).collect::<Vec<_>>(),
            "schedule": look.schedule.iter().map(block_json).collect::<Vec<_>>(),
        }));
    }

    if look.events.is_empty() && look.schedule.is_empty() {
        return None;
    }

    Some(json!({
        "focus": "events",
        "events": look.events.iter().map(event_json).collect::<Vec<_>>(),
        "schedule": look.schedule.iter().map(block_json).collect::<Vec<_>>(),
        "off_days": look.off_days,
    }))
}

fn parse_event(value: &Value) -> Option<LastLookEvent> {
    let title = value.get("title")?.as_str()?.to_string();
    let start = value
        .get("start")
        .or_else(|| value.get("start_time"))
        .and_then(value_i64)?;
    let end = value
        .get("end")
        .or_else(|| value.get("end_time"))
        .and_then(value_i64)
        .unwrap_or(start);
    Some(LastLookEvent {
        id: value
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        title,
        start,
        end,
        all_day: value
            .get("all_day")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        location: value
            .get("location")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    })
}

fn parse_slot(value: &Value) -> Option<LastLookSlot> {
    Some(LastLookSlot {
        start: value.get("start").and_then(value_i64)?,
        end: value.get("end").and_then(value_i64)?,
    })
}

fn parse_block(value: &Value) -> Option<LastLookBlock> {
    Some(LastLookBlock {
        kind: value
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("work")
            .to_string(),
        title: value
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Schedule")
            .to_string(),
        start: value.get("start").and_then(value_i64)?,
        end: value.get("end").and_then(value_i64)?,
    })
}

fn block_json(block: &LastLookBlock) -> Value {
    json!({
        "kind": block.kind,
        "title": block.title,
        "start": block.start,
        "end": block.end,
    })
}

fn schedule_followup_json(look: &LastLook) -> Value {
    json!({
        "focus": "work",
        "when": look.when,
        "schedule": look
            .schedule
            .iter()
            .filter(|b| b.kind == "work")
            .map(block_json)
            .collect::<Vec<_>>(),
        "off_days": look.off_days,
        "events": [],
    })
}

fn is_working_query(lower: &str) -> bool {
    lower.contains("when am i working")
        || lower.contains("when do i work")
        || lower.contains("what time do i work")
        || lower.contains("when do i finish work")
        || lower.contains("when does work end")
        || lower.contains("when am i at work")
        || lower.contains("what are my work hours")
}

fn value_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_f64().map(|f| f as i64))
        .or_else(|| value.as_str()?.parse().ok())
}

fn event_json(event: &LastLookEvent) -> Value {
    json!({
        "id": event.id,
        "title": event.title,
        "start": event.start,
        "end": event.end,
        "all_day": event.all_day,
        "location": event.location,
    })
}

fn match_title<'a>(look: &'a LastLook, lower: &str) -> Option<&'a LastLookEvent> {
    look.events.iter().find(|event| {
        let title = event.title.to_ascii_lowercase();
        !title.is_empty() && lower.contains(&title)
    })
}

fn looks_like_fresh_look(lower: &str) -> bool {
    lower.contains("what's on")
        || lower.contains("whats on")
        || lower.contains("what is on")
        || lower.contains("what's planned")
        || lower.contains("whats planned")
        || lower.contains("what is planned")
        || lower.contains("when am i free")
        || lower.contains("on my calendar")
        || lower.contains("anything today")
        || lower.contains("anything tomorrow")
        || lower.contains("anything this week")
        || lower.contains("anything next week")
        || lower.contains("next week")
        || lower.contains("this week")
        || is_working_query(lower)
}

fn is_when_followup(lower: &str) -> bool {
    let t = lower.trim().trim_end_matches(['?', '.', '!']).trim();
    matches!(
        t,
        "when was it"
            | "when is it"
            | "when is that"
            | "when was that"
            | "what time"
            | "what time is it"
            | "what time was it"
            | "what time is that"
            | "what time was that"
            | "which day"
            | "what day"
            | "what day is it"
            | "what day was it"
            | "when"
            | "and when"
    ) || t.starts_with("when was it")
        || t.starts_with("when is that")
        || t.starts_with("what time was")
        || t.starts_with("what time is that")
}

fn is_when_ish(lower: &str) -> bool {
    is_when_followup(lower)
        || lower.contains("what time")
        || lower.contains("what day")
        || lower.contains("which day")
        || lower.contains("when is")
        || lower.contains("when was")
        || lower.contains("when's")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_look() -> LastLook {
        LastLook {
            when: Some("this_week".into()),
            focus: "events".into(),
            events: vec![
                LastLookEvent {
                    id: "1".into(),
                    title: "Dentist".into(),
                    start: 1_786_039_200_000,
                    end: 1_786_042_800_000,
                    all_day: false,
                    location: None,
                },
                LastLookEvent {
                    id: "2".into(),
                    title: "Climbing".into(),
                    start: 1_786_125_600_000,
                    end: 1_786_131_000_000,
                    all_day: false,
                    location: None,
                },
            ],
            slots: vec![],
            schedule: vec![LastLookBlock {
                kind: "work".into(),
                title: "Work".into(),
                start: 1_786_033_500_000,
                end: 1_786_062_300_000,
            }],
            off_days: vec![],
        }
    }

    #[test]
    fn when_am_i_working_is_fresh_look() {
        assert!(select_followup_payload(&sample_look(), "when am I working?").is_none());
    }

    #[test]
    fn when_was_it_returns_all_last_events() {
        let payload = select_followup_payload(&sample_look(), "when was it?").unwrap();
        let events = payload.get("events").and_then(|v| v.as_array()).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn when_is_the_dentist_picks_title() {
        let payload = select_followup_payload(&sample_look(), "when is the dentist?").unwrap();
        let events = payload.get("events").and_then(|v| v.as_array()).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].get("title").and_then(|v| v.as_str()),
            Some("Dentist")
        );
    }

    #[test]
    fn whats_on_today_is_not_a_followup() {
        assert!(select_followup_payload(&sample_look(), "whats on today?").is_none());
        assert!(select_followup_payload(&sample_look(), "when am I free tomorrow?").is_none());
        assert!(select_followup_payload(&sample_look(), "next week plans?").is_none());
    }
}
