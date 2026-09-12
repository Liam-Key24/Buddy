//! Personality controls *how* Buddy communicates.
//!
//! It does not plan, execute tools, or store memory. Clarification decides
//! *what* to ask; this crate only phrases the ask and lightly styles replies.

use serde::{Deserialize, Serialize};
use serde_json::Value;

mod view_model;
pub use view_model::{view_model_from_json, Fact, ToolViewModel};

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
        Err(_) => phrase_plain_tool_output(trimmed).unwrap_or_else(|| trimmed.to_string()),
    }
}

fn phrase_plain_tool_output(trimmed: &str) -> Option<String> {
    if let Some(rest) = trimmed.strip_prefix("Added todo ") {
        if let Some((_, title)) = rest.split_once(" — ") {
            let title = title.trim();
            if !title.is_empty() {
                return Some(format!("Added “{title}”."));
            }
        }
    }
    None
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
    if let Some(vm) = view_model::view_model_from_json(tool, value) {
        return Some(vm.render());
    }
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
        // Batch create returns [{status, event}, ...] — not a search/list result.
        if tool.contains("create") || tool.contains("duplicate") {
            return Some(phrase_created_batch(arr));
        }
        return Some(phrase_list(tool, arr));
    }

    if let Some(obj) = value.as_object() {
        if tool == "docs.get" || (tool.starts_with("docs.") && obj.get("content").is_some()) {
            return Some(phrase_document(obj));
        }
        if tool.starts_with("docs.") {
            if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                let action = match obj.get("action").and_then(|v| v.as_str()) {
                    Some("created") => "Created",
                    Some("updated") => "Updated",
                    Some("deleted") => "Deleted",
                    Some("formatted") => "Reformatted",
                    Some("patched") => "Updated",
                    _ => "Saved",
                };
                return Some(format!("{action} document “{title}”."));
            }
        }
        if tool == "fitness.look" || tool == "fitness.summary" {
            return Some(phrase_fitness_look(obj));
        }
        if tool == "fitness.log_food" {
            return Some(phrase_food_logged(obj));
        }
        if tool == "fitness.fridge" {
            if let Some(name) = obj.get("name").and_then(|n| n.as_str()).filter(|s| !s.is_empty()) {
                return Some(format!("Added {name} to the fridge."));
            }
        }
        if tool == "fitness.log_workout" {
            return Some(phrase_workout_logged(obj));
        }
        if tool == "fitness.log_weight" {
            if let Some(kg) = obj.get("kg").and_then(|v| v.as_f64()) {
                return Some(format!("Logged weight {kg:.1}kg."));
            }
        }
        if tool.starts_with("study.upsert") {
            if let Some(msg) = phrase_study_upsert(tool, obj) {
                return Some(msg);
            }
        }
        if tool == "money.list" {
            if let Some(entries) = obj.get("entries").and_then(|e| e.as_array()) {
                return Some(phrase_money_entries(entries));
            }
        }
        if tool == "money.log" {
            return Some(phrase_money_log(obj));
        }
        if tool == "money.summary" {
            return Some(phrase_money_summary(obj));
        }
        if tool == "money.pot" {
            return Some(phrase_money_pot(obj));
        }
        if tool == "money.pots" {
            if let Some(pots) = obj.get("pots").and_then(|e| e.as_array()) {
                return Some(phrase_money_pots(pots, obj));
            }
        }
        if tool == "study.look" || tool == "study.status" || tool == "study.log_session" {
            if let Some(msg) = phrase_study_object(obj) {
                return Some(msg);
            }
        }
        if tool == "calendar.look" || tool.contains("find_free_time") {
            if tool == "calendar.look" {
                return Some(phrase_look_snapshot(obj));
            }
            let focus = obj.get("focus").and_then(|v| v.as_str()).unwrap_or("");
            if focus == "free" || tool.contains("find_free_time") {
                if let Some(arr) = obj.get("slots").and_then(|s| s.as_array()) {
                    return Some(phrase_free_slots(arr));
                }
            }
            if let Some(arr) = obj.get("events").and_then(|s| s.as_array()) {
                return Some(phrase_agenda(arr));
            }
        }
        // Conflict-aware create/update: { status: "ok"|"conflict", ... }
        if let Some(status) = obj.get("status").and_then(|v| v.as_str()) {
            if status == "conflict" {
                return Some(phrase_conflict(obj));
            }
            if status == "ok" {
                if let Some(event) = obj.get("event") {
                    return Some(phrase_created_one(event));
                }
            }
        }

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
            let when = phrase_time_range(
                obj.get("start").or_else(|| obj.get("start_time")),
                obj.get("end").or_else(|| obj.get("end_time")),
            );
            return Some(format!("{action} “{title}”{loc}{when}."));
        }

        if tool.contains("stats") || obj.contains_key("total_sales") || obj.contains_key("hours")
        {
            return Some(format_object_summary(obj));
        }
    }

    None
}

fn phrase_created_one(event: &Value) -> String {
    let title = event
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("event");
    let loc = event
        .get("location")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| format!(" at {s}"))
        .unwrap_or_default();
    let when = phrase_time_range(
        event.get("start").or_else(|| event.get("start_time")),
        event.get("end").or_else(|| event.get("end_time")),
    );
    format!("Created “{title}”{loc}{when}.")
}

fn phrase_created_batch(items: &[Value]) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut conflicts = 0usize;
    for item in items {
        if let Some(obj) = item.as_object() {
            if obj.get("status").and_then(|v| v.as_str()) == Some("conflict") {
                conflicts += 1;
                continue;
            }
            let event = obj.get("event").unwrap_or(item);
            if let Some(title) = event.get("title").and_then(|v| v.as_str()) {
                let when = phrase_time_range(
                    event.get("start").or_else(|| event.get("start_time")),
                    event.get("end").or_else(|| event.get("end_time")),
                );
                lines.push(format!("“{title}”{when}"));
                continue;
            }
        }
        if let Some(title) = item.get("title").and_then(|v| v.as_str()) {
            let when = phrase_time_range(
                item.get("start").or_else(|| item.get("start_time")),
                item.get("end").or_else(|| item.get("end_time")),
            );
            lines.push(format!("“{title}”{when}"));
        }
    }

    if lines.is_empty() {
        return if conflicts > 0 {
            format!("Couldn’t add {conflicts} events — time conflicts.")
        } else {
            "Created the events.".into()
        };
    }

    let listed = join_natural(&lines);
    let mut msg = if lines.len() == 1 {
        format!("Created {listed}.")
    } else {
        format!("Created {listed}.")
    };
    if conflicts > 0 {
        msg.push_str(&format!(" ({conflicts} hit conflicts.)"));
    }
    msg
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

    // Shared propose/confirm envelope: schedule_task, block_time, plan_day.
    if tool.contains("schedule_task")
        || tool.contains("block_time")
        || tool.contains("plan_day")
        || obj.contains_key("scheduled")
        || obj.contains_key("proposed")
    {
        return Some(phrase_proposal_result(obj));
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

/// Shared phrasing for schedule_task / block_time / plan_day proposal envelopes.
fn phrase_proposal_result(obj: &serde_json::Map<String, Value>) -> String {
    let applied = obj.get("apply").and_then(|v| v.as_bool()) == Some(true)
        || obj
            .get("status")
            .and_then(|v| v.as_str())
            .map(|s| s.eq_ignore_ascii_case("applied") || s.eq_ignore_ascii_case("scheduled"))
            .unwrap_or(false);

    let mut blocks: Vec<&Value> = obj
        .get("scheduled")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    if blocks.is_empty() {
        blocks = obj
            .get("proposed")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().collect())
            .unwrap_or_default();
    }
    blocks.sort_by_key(|p| {
        p.get("start")
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
            .unwrap_or(0)
    });

    let unscheduled = obj
        .get("unscheduled")
        .and_then(|v| v.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[]);

    let mut parts = Vec::new();
    if !blocks.is_empty() {
        let lines: Vec<String> = blocks
            .iter()
            .filter_map(|s| {
                let title = s.get("title")?.as_str()?;
                let when = phrase_time_range(s.get("start"), s.get("end"));
                Some(format!("“{title}”{when}"))
            })
            .take(6)
            .collect();
        let more = if blocks.len() > 6 {
            format!(" (+{} more)", blocks.len() - 6)
        } else {
            String::new()
        };
        let verb = if applied { "Added" } else { "Proposed" };
        parts.push(format!(
            "{verb} {}: {}{more}.",
            if lines.len() == 1 {
                "1 block".into()
            } else {
                format!("{} blocks", lines.len())
            },
            join_natural(&lines)
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
        if !applied && !blocks.is_empty() {
            parts.push(if blocks.len() == 1 {
                "Add this to your calendar?".into()
            } else {
                "Add these to your calendar?".into()
            });
        }
        parts.join(" ")
    }
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
    let base = match alt {
        Some(a) => format!("{msg} {a}"),
        None => msg.to_string(),
    };
    format!("{base} Say allow to book it anyway.")
}

fn phrase_time_range(start: Option<&Value>, end: Option<&Value>) -> String {
    let Some(s) = start.and_then(value_as_i64) else {
        return String::new();
    };
    let Some(e) = end.and_then(value_as_i64) else {
        return format!(" on {}", format_local_datetime(s));
    };
    if same_local_day(s, e) {
        format!(
            " on {}–{}",
            format_local_datetime(s),
            format_local_clock(e)
        )
    } else {
        format!(
            " from {} to {}",
            format_local_datetime(s),
            format_local_datetime(e)
        )
    }
}

fn value_as_i64(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_f64().map(|f| f as i64))
        .or_else(|| v.as_str()?.parse().ok())
}

fn same_local_day(a: i64, b: i64) -> bool {
    use chrono::{Local, TimeZone};
    let da = Local.timestamp_millis_opt(a).single().map(|d| d.date_naive());
    let db = Local.timestamp_millis_opt(b).single().map(|d| d.date_naive());
    da.is_some() && da == db
}

/// Weekday + short date + clock, e.g. "Wed 6 Aug, 6:00 PM".
fn format_local_datetime(ms: i64) -> String {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|dt| {
            let date = dt.format("%a %-d %b").to_string();
            let clock = dt
                .format("%-I:%M %p")
                .to_string();
            format!("{date}, {clock}")
        })
        .unwrap_or_else(|| ms.to_string())
}

fn format_local_clock(ms: i64) -> String {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|dt| dt.format("%-I:%M %p").to_string())
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

fn phrase_fitness_look(obj: &serde_json::Map<String, Value>) -> String {
    if let Some(workouts) = obj.get("workouts").and_then(|v| v.as_array()) {
        return phrase_workouts(workouts);
    }
    if let Some(foods) = obj.get("foods").and_then(|v| v.as_array()) {
        let date = obj.get("date").and_then(|d| d.as_str()).unwrap_or("today");
        if foods.is_empty() {
            return format!("Nothing logged for {date}.");
        }
        let bits: Vec<String> = foods
            .iter()
            .filter_map(|f| {
                let name = f.get("name")?.as_str()?;
                let kcal = f.get("calories").and_then(|c| c.as_f64()).unwrap_or(0.0);
                Some(format!("{name} ({kcal:.0} kcal)"))
            })
            .take(8)
            .collect();
        return format!("Food {date}: {}.", join_natural(&bits));
    }
    if let Some(weight) = obj.get("weight").and_then(|v| v.as_array()) {
        if weight.is_empty() {
            return "No weigh-ins logged.".into();
        }
        let bits: Vec<String> = weight
            .iter()
            .filter_map(|w| {
                let kg = w.get("kg")?.as_f64()?;
                let date = w.get("date").and_then(|d| d.as_str()).unwrap_or("");
                Some(if date.is_empty() {
                    format!("{kg:.1}kg")
                } else {
                    format!("{date} {kg:.1}kg")
                })
            })
            .take(8)
            .collect();
        return format!("Weight: {}.", join_natural(&bits));
    }
    if let Some(climbs) = obj.get("climbs").and_then(|v| v.as_array()) {
        if climbs.is_empty() {
            return "No climbs logged.".into();
        }
        let bits: Vec<String> = climbs
            .iter()
            .filter_map(|c| {
                let name = c.get("name").and_then(|n| n.as_str()).unwrap_or("Problem");
                let grade = c.get("grade").and_then(|g| g.as_str()).unwrap_or("");
                Some(format!("{name} {grade}").trim().to_string())
            })
            .take(8)
            .collect();
        return format!("Climbs: {}.", join_natural(&bits));
    }
    if let Some(fridge) = obj.get("fridge").and_then(|v| v.as_array()) {
        if fridge.is_empty() {
            return "Fridge is empty.".into();
        }
        let bits: Vec<String> = fridge
            .iter()
            .filter_map(|i| i.get("name")?.as_str().map(|s| s.to_string()))
            .take(10)
            .collect();
        return format!("Fridge: {}.", join_natural(&bits));
    }
    format_object_summary(obj)
}

fn phrase_workouts(workouts: &[Value]) -> String {
    if workouts.is_empty() {
        return "No workouts logged.".into();
    }
    let mut lines = vec![format!(
        "{} workout{}:",
        workouts.len(),
        if workouts.len() == 1 { "" } else { "s" }
    )];
    for w in workouts.iter().take(8) {
        let name = w
            .get("name")
            .and_then(|n| n.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("Workout");
        let date = w.get("date").and_then(|d| d.as_str()).unwrap_or("");
        let sets = w.get("sets").and_then(|s| s.as_array()).cloned().unwrap_or_default();
        let set_bits: Vec<String> = sets
            .iter()
            .filter_map(|s| {
                let ex = s.get("exercise")?.as_str()?;
                let reps = s.get("reps").and_then(|r| r.as_i64());
                let wt = s.get("weight").and_then(|w| w.as_f64());
                match (reps, wt) {
                    (Some(r), Some(kg)) => Some(format!("{ex} {r}×{kg}kg")),
                    (Some(r), None) => Some(format!("{ex} ×{r}")),
                    (None, Some(kg)) => Some(format!("{ex} {kg}kg")),
                    _ => Some(ex.to_string()),
                }
            })
            .collect();
        let head = if date.is_empty() {
            name.to_string()
        } else {
            format!("{date} {name}")
        };
        if set_bits.is_empty() {
            lines.push(format!("• {head}"));
        } else {
            lines.push(format!("• {head}: {}", set_bits.join(", ")));
        }
    }
    lines.join("\n")
}

fn phrase_money_entries(entries: &[Value]) -> String {
    if entries.is_empty() {
        return "No money entries for that month.".into();
    }
    let bits: Vec<String> = entries
        .iter()
        .filter_map(|e| {
            let desc = e.get("description")?.as_str()?;
            let amount = e.get("amount").and_then(|a| a.as_f64()).unwrap_or_else(|| {
                e.get("amount_cents")
                    .and_then(|c| c.as_i64())
                    .map(|c| c as f64 / 100.0)
                    .unwrap_or(0.0)
            });
            let kind = e.get("kind").and_then(|k| k.as_str()).unwrap_or("expense");
            Some(format!("{desc} £{amount:.2} ({kind})"))
        })
        .take(8)
        .collect();
    format!("{}: {}.", entries.len(), join_natural(&bits))
}

fn phrase_money_log(obj: &serde_json::Map<String, Value>) -> String {
    let desc = obj
        .get("description")
        .and_then(|d| d.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("entry");
    let pounds = obj
        .get("amount")
        .and_then(|a| a.as_f64())
        .unwrap_or_else(|| {
            obj.get("amount_cents")
                .and_then(|c| c.as_i64())
                .map(|c| c as f64 / 100.0)
                .unwrap_or(0.0)
        });
    let kind = obj.get("kind").and_then(|k| k.as_str()).unwrap_or("expense");
    format!("Logged {kind} £{pounds:.2} — {desc}.")
}

fn phrase_money_summary(obj: &serde_json::Map<String, Value>) -> String {
    let income = cents_to_pounds(obj.get("income_cents"));
    let expense = cents_to_pounds(obj.get("expense_cents"));
    let net = cents_to_pounds(obj.get("net_cents"));
    let mut msg = format!("This month: £{income:.2} in, £{expense:.2} out, net £{net:.2}.");
    if let Some(pots) = obj.get("pots").and_then(|v| v.as_array()) {
        if !pots.is_empty() {
            msg.push(' ');
            msg.push_str(&phrase_money_pots(pots, obj));
        }
    }
    msg
}

fn phrase_money_pot(obj: &serde_json::Map<String, Value>) -> String {
    let name = obj
        .get("name")
        .and_then(|n| n.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("savings");
    let pounds = obj
        .get("amount")
        .and_then(|a| a.as_f64())
        .unwrap_or_else(|| {
            obj.get("balance_cents")
                .and_then(|c| c.as_i64())
                .map(|c| c as f64 / 100.0)
                .unwrap_or(0.0)
        });
    let mode = obj.get("mode").and_then(|m| m.as_str()).unwrap_or("set");
    if mode == "add" {
        format!("Added to {name}. That pot is now £{pounds:.2}.")
    } else {
        format!("{name} pot is £{pounds:.2}.")
    }
}

fn phrase_money_pots(pots: &[Value], obj: &serde_json::Map<String, Value>) -> String {
    if pots.is_empty() {
        return "No savings pots yet.".into();
    }
    let total = obj
        .get("amount")
        .and_then(|a| a.as_f64())
        .unwrap_or_else(|| cents_to_pounds(obj.get("pots_cents")));
    let bits: Vec<String> = pots
        .iter()
        .filter_map(|p| {
            let name = p.get("name")?.as_str()?;
            let amount = p.get("amount").and_then(|a| a.as_f64()).unwrap_or_else(|| {
                p.get("balance_cents")
                    .and_then(|c| c.as_i64())
                    .map(|c| c as f64 / 100.0)
                    .unwrap_or(0.0)
            });
            let pct = p.get("pct").and_then(|v| v.as_f64()).unwrap_or(0.0);
            if pct > 0.0 {
                Some(format!("{name} £{amount:.2} ({pct:.0}%)"))
            } else {
                Some(format!("{name} £{amount:.2}"))
            }
        })
        .collect();
    if total > 0.0 {
        format!("Savings split £{total:.2}: {}.", join_natural(&bits))
    } else {
        format!("Savings pots: {}.", join_natural(&bits))
    }
}

fn cents_to_pounds(v: Option<&Value>) -> f64 {
    v.and_then(|c| c.as_i64()).map(|c| c as f64 / 100.0).unwrap_or(0.0)
}

fn phrase_food_logged(obj: &serde_json::Map<String, Value>) -> String {
    let name = obj
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("food");
    let kcal = obj.get("calories").and_then(|c| c.as_f64()).unwrap_or(0.0);
    format!("Logged {name} ({kcal:.0} kcal).")
}

fn phrase_workout_logged(obj: &serde_json::Map<String, Value>) -> String {
    if let Some(w) = obj.get("workout").and_then(|v| v.as_object()) {
        let name = w.get("name").and_then(|n| n.as_str()).unwrap_or("Workout");
        let sets = w.get("sets").and_then(|s| s.as_array()).cloned().unwrap_or_default();
        if sets.is_empty() {
            return format!("Logged workout “{name}”.");
        }
        return format!("Logged workout “{name}” ({} set{}).", sets.len(), if sets.len() == 1 { "" } else { "s" });
    }
    if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
        return format!("Logged workout “{name}”.");
    }
    "Logged workout.".into()
}

fn phrase_document(obj: &serde_json::Map<String, Value>) -> String {
    let title = obj
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("Document");
    let content = obj.get("content").and_then(|c| c.as_str()).unwrap_or("").trim();
    if content.is_empty() {
        return format!("Document “{title}” is empty.");
    }
    let preview: String = content.chars().take(420).collect();
    let extra = content.chars().count() > preview.chars().count();
    if extra {
        format!("Document “{title}”:\n{preview}…")
    } else {
        format!("Document “{title}”:\n{preview}")
    }
}

fn phrase_study_upsert(tool: &str, obj: &serde_json::Map<String, Value>) -> Option<String> {
    let noun = if tool.contains("assignment") {
        "assignment"
    } else if tool.contains("topic") {
        "topic"
    } else {
        "subject"
    };
    let label = obj
        .get("title")
        .or_else(|| obj.get("name"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())?;
    Some(format!("Saved study {noun} “{label}”."))
}

fn phrase_study_object(obj: &serde_json::Map<String, Value>) -> Option<String> {
    if obj.contains_key("recent_sessions") || obj.contains_key("minutes_per_day") {
        let mins = obj
            .get("minutes_per_day")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let topics = obj
            .get("topics")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        let assigns = obj
            .get("assignments")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        return Some(format!(
            "Study pace {mins:.0} min/day · {topics} topics · {assigns} assignments."
        ));
    }
    if let Some(mins) = obj.get("duration_minutes").and_then(|v| v.as_i64()) {
        let date = obj.get("date").and_then(|d| d.as_str()).unwrap_or("today");
        return Some(format!("Logged {mins} min study session ({date})."));
    }
    None
}

fn phrase_look_snapshot(obj: &serde_json::Map<String, Value>) -> String {
    let focus = obj.get("focus").and_then(|v| v.as_str()).unwrap_or("");
    let when_label = obj.get("when").and_then(|v| v.as_str()).unwrap_or("");
    let events = obj
        .get("events")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    let schedule = obj
        .get("schedule")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    let off_days = obj
        .get("off_days")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    let slots = obj
        .get("slots")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();

    if focus == "work" {
        return phrase_work_hours(&schedule, when_label, &off_days);
    }
    if focus == "sleep" {
        return phrase_schedule_kind(&schedule, "sleep", "Sleep")
            .unwrap_or_else(|| "No sleep schedule in that stretch.".into());
    }

    let mut parts: Vec<String> = Vec::new();
    if !off_days.is_empty() {
        let days: Vec<String> = off_days
            .iter()
            .filter_map(|d| d.as_str().map(|s| s.to_string()))
            .collect();
        if !days.is_empty() {
            parts.push(format!("Off / holiday: {}.", join_natural(&days)));
        }
    }
    if let Some(line) = phrase_schedule_kind(&schedule, "work", "Work") {
        parts.push(line);
    }
    let sleep_n = schedule
        .iter()
        .filter(|b| b.get("kind").and_then(|k| k.as_str()) == Some("sleep"))
        .count();
    // Overnight + evening on a single day is fine; a week of mixed nights is noise.
    if sleep_n > 0 && sleep_n <= 2 {
        if let Some(line) = phrase_schedule_kind(&schedule, "sleep", "Sleep") {
            parts.push(line);
        }
    }

    if focus == "free" || focus == "slots" {
        parts.push(phrase_free_slots(&slots));
        if !events.is_empty() {
            parts.push(phrase_agenda(&events));
        }
        return parts.join("\n\n");
    }

    if events.is_empty() {
        if parts.is_empty() {
            return "Nothing on your calendar for that stretch.".into();
        }
        return parts.join("\n\n");
    }
        parts.push(phrase_agenda(&events));
    parts.join("\n\n")
}

fn phrase_work_hours(schedule: &[Value], when_label: &str, off_days: &[Value]) -> String {
    let work: Vec<&Value> = schedule
        .iter()
        .filter(|b| b.get("kind").and_then(|k| k.as_str()) == Some("work"))
        .collect();
    let not_in = match when_label {
        "today" => "You're not working today.",
        "tomorrow" => "You're not working tomorrow.",
        _ => "You're not working that stretch.",
    };
    if work.is_empty() {
        return not_in.into();
    }

    let Some(line) = phrase_schedule_kind(schedule, "work", "Work") else {
        return not_in.into();
    };

    let first_start = work[0].get("start").and_then(value_as_i64);
    let outside_named_day = matches!(when_label, "today" | "tomorrow")
        && first_start.is_some_and(|ms| {
            let day = local_date_key(ms);
            let today = local_date_key(chrono::Local::now().timestamp_millis());
            if when_label == "today" {
                day != today
            } else {
                day != today_plus_days(1)
            }
        });
    if outside_named_day {
        let when = phrase_time_range(work[0].get("start"), work[0].get("end"));
        return format!("{not_in} Next is{when}.");
    }

    let mut out = line;
    if !off_days.is_empty() {
        let days: Vec<String> = off_days
            .iter()
            .filter_map(|d| d.as_str().map(|s| s.to_string()))
            .collect();
        if !days.is_empty() {
            out.push_str(&format!(" Off / holiday: {}.", join_natural(&days)));
        }
    }
    out
}

fn local_date_key(ms: i64) -> String {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

fn today_plus_days(n: i64) -> String {
    use chrono::{Duration, Local};
    (Local::now().date_naive() + Duration::days(n))
        .format("%Y-%m-%d")
        .to_string()
}

fn phrase_schedule_kind(blocks: &[Value], kind: &str, label: &str) -> Option<String> {
    let items: Vec<&Value> = blocks
        .iter()
        .filter(|b| b.get("kind").and_then(|k| k.as_str()) == Some(kind))
        .collect();
    if items.is_empty() {
        return None;
    }
    if items.len() == 1 {
        let when = phrase_time_range(items[0].get("start"), items[0].get("end"));
        if when.is_empty() {
            return Some(format!("**{label}** is on your schedule."));
        }
        return Some(format!("**{label}** —{when}"));
    }
    let mut clocks: Vec<(String, String)> = Vec::new();
    for b in &items {
        let Some(s) = b.get("start").and_then(value_as_i64) else {
            continue;
        };
        let Some(e) = b.get("end").and_then(value_as_i64) else {
            continue;
        };
        let pair = (format_local_clock(s), format_local_clock(e));
        if !clocks.iter().any(|c| c == &pair) {
            clocks.push(pair);
        }
    }
    if clocks.len() == 1 {
        if kind == "work" && items.len() >= 4 {
            return Some(format!("**{label}** — weekdays {}–{}", clocks[0].0, clocks[0].1));
        }
        return Some(format!("**{label}** — {}–{}", clocks[0].0, clocks[0].1));
    }
    Some(format!("{label} varies across those days."))
}

fn phrase_agenda(items: &[Value]) -> String {
    if items.is_empty() {
        return "Nothing on your calendar for that stretch.".into();
    }
    if items.len() == 1 {
        let title = item_title(&items[0]).unwrap_or("That");
        let when = phrase_time_range(
            items[0].get("start").or_else(|| items[0].get("start_time")),
            items[0].get("end").or_else(|| items[0].get("end_time")),
        );
        if when.is_empty() {
            return format!("{title} is on your calendar.");
        }
        return format!("{title} is{when}.");
    }

    let mut lines = vec![format!("**{} on your calendar**", items.len())];
    for item in items.iter().take(12) {
        let title = item_title(item).unwrap_or("Event");
        let when = phrase_time_range(
            item.get("start").or_else(|| item.get("start_time")),
            item.get("end").or_else(|| item.get("end_time")),
        );
        if when.is_empty() {
            lines.push(format!("- {title}"));
        } else {
            lines.push(format!("- {title}{when}"));
        }
    }
    let extra = items.len().saturating_sub(12);
    if extra > 0 {
        lines.push(format!("- …and {extra} more"));
    }
    lines.join("\n")
}

fn item_title(item: &Value) -> Option<&str> {
    item.get("title")
        .or_else(|| item.get("name"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}

fn list_noun(tool: &str, items: &[Value]) -> &'static str {
    if tool.starts_with("calendar.") {
        return "event";
    }
    if tool == "todo.list" || tool.starts_with("todo.") {
        return "task";
    }
    if tool.starts_with("docs.") {
        return "document";
    }
    if tool.contains("spark") {
        return "spark";
    }
    if tool.starts_with("research.") {
        return "research session";
    }
    if tool.starts_with("socials.") {
        return "post";
    }
    if tool.starts_with("study.") {
        if items.iter().any(|i| i.get("duration_minutes").is_some()) {
            return "study session";
        }
        if items.iter().any(|i| i.get("kind").is_some() && i.get("title").is_some()) {
            return "assignment";
        }
        if items.iter().any(|i| i.get("last_studied").is_some() || i.get("remaining_estimate").is_some()) {
            return "topic";
        }
        return "subject";
    }
    if tool.contains("dream") {
        return "dream";
    }
    if tool.contains("block") {
        return "block";
    }
    "item"
}

fn item_label(item: &Value) -> Option<String> {
    if let Some(s) = item
        .get("title")
        .or_else(|| item.get("name"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        return Some(s.to_string());
    }
    if let Some(mins) = item.get("duration_minutes").and_then(|v| v.as_i64()) {
        let date = item.get("date").and_then(|d| d.as_str()).unwrap_or("");
        let notes = item
            .get("notes")
            .and_then(|n| n.as_str())
            .filter(|s| !s.is_empty());
        let head = if date.is_empty() {
            format!("{mins} min")
        } else {
            format!("{date} — {mins} min")
        };
        return Some(match notes {
            Some(n) => format!("{head} ({n})"),
            None => head,
        });
    }
    if let Some(s) = item
        .get("snippet")
        .or_else(|| item.get("body"))
        .or_else(|| item.get("content"))
        .or_else(|| item.get("question"))
        .or_else(|| item.get("description"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let preview: String = s.chars().take(80).collect();
        return Some(preview);
    }
    None
}

fn phrase_list(tool: &str, items: &[Value]) -> String {
    let noun = list_noun(tool, items);

    if items.is_empty() {
        return format!("No {noun}s found.");
    }

    let titles: Vec<String> = items.iter().filter_map(item_label).take(5).collect();

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
    fn phrases_fitness_workouts() {
        let msg = phrase_tool_result(
            "fitness.look",
            r#"{"workouts":[{"name":"Push","date":"2026-08-08","sets":[{"exercise":"Bench Press","reps":8,"weight":70}]}]}"#,
        );
        assert!(msg.contains("Push"), "{msg}");
        assert!(msg.contains("Bench Press"), "{msg}");
        assert!(msg.contains("70"), "{msg}");
        assert!(!msg.contains("calendar"), "{msg}");
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
    fn phrases_batch_created_events_not_found() {
        let msg = phrase_tool_result(
            "calendar.create_event",
            r#"[
              {"status":"ok","event":{"title":"study","start_time":1785052800000,"end_time":1785063600000}},
              {"status":"ok","event":{"title":"research","start_time":1785063600000,"end_time":1785070800000}}
            ]"#,
        );
        assert!(msg.contains("Created"));
        assert!(msg.contains("study"));
        assert!(msg.contains("research"));
        assert!(!msg.contains("Found"));
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
    fn phrases_schedule_task_propose_asks_confirm() {
        // 2026-08-06 18:00–20:00 local-ish via fixed millis (UTC); phrasing uses Local.
        let msg = phrase_tool_result(
            "calendar.schedule_task",
            r#"{"apply":false,"scheduled":[{"title":"Climbing","start":1786039200000,"end":1786046400000}]}"#,
        );
        assert!(msg.contains("Climbing"));
        assert!(msg.contains("Add this to your calendar?"));
        // Dated proposal: weekday or month token, not clock-only.
        let has_date = msg.contains("Aug")
            || msg.contains("Jul")
            || msg.contains("Mon")
            || msg.contains("Tue")
            || msg.contains("Wed")
            || msg.contains("Thu")
            || msg.contains("Fri")
            || msg.contains("Sat")
            || msg.contains("Sun");
        assert!(has_date, "expected weekday/date in proposal: {msg}");
    }

    #[test]
    fn phrases_block_time() {
        let msg = phrase_tool_result(
            "calendar.block_time",
            r#"{"apply":false,"scheduled":[{"title":"Coding","start":1784800000000,"end":1784810800000}],"unscheduled":[]}"#,
        );
        assert!(msg.contains("Coding"));
        assert!(msg.contains("Proposed"));
        assert!(msg.contains("Add this to your calendar?"));
    }

    #[test]
    fn phrases_plan_day_applied_no_confirm() {
        let msg = phrase_tool_result(
            "calendar.plan_day",
            r#"{"apply":true,"proposed":[{"title":"Gym","start":1,"end":2},{"title":"Bath","start":3,"end":4}],"unscheduled":[]}"#,
        );
        assert!(msg.contains("Gym"));
        assert!(msg.contains("Added"));
        assert!(!msg.contains("Add these to your calendar?"));
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

    #[test]
    fn phrases_look_agenda_with_times() {
        let msg = phrase_tool_result(
            "calendar.look",
            r#"{"focus":"events","events":[{"id":"1","title":"Dentist","start":1786039200000,"end":1786042800000}]}"#,
        );
        assert!(msg.contains("Dentist"));
        assert!(msg.contains("is"));
        assert!(!msg.contains("Found 1 event"));
        assert!(!msg.contains("\"start\""));
    }

    #[test]
    fn phrases_look_includes_work_schedule() {
        let msg = phrase_tool_result(
            "calendar.look",
            r#"{"focus":"events","events":[],"schedule":[{"id":"work::2026-08-10","kind":"work","title":"Work","start":1786345500000,"end":1786374300000,"anchor_date":"2026-08-10"}],"off_days":[]}"#,
        );
        assert!(msg.contains("Work"));
        assert!(!msg.contains("Nothing on your calendar"));
    }

    #[test]
    fn phrases_work_focus_skips_sleep() {
        let msg = phrase_tool_result(
            "calendar.look",
            r#"{"focus":"work","when":"this_week","events":[],"schedule":[
              {"kind":"work","title":"Work","start":1786345500000,"end":1786374300000},
              {"kind":"work","title":"Work","start":1786431900000,"end":1786460700000},
              {"kind":"work","title":"Work","start":1786518300000,"end":1786547100000},
              {"kind":"work","title":"Work","start":1786604700000,"end":1786633500000},
              {"kind":"work","title":"Work","start":1786691100000,"end":1786719900000}
            ],"off_days":[]}"#,
        );
        assert!(msg.contains("Work"));
        assert!(!msg.to_ascii_lowercase().contains("sleep"));
        assert!(!msg.contains("varies"));
    }

    #[test]
    fn phrases_work_focus_empty_today() {
        let msg = phrase_tool_result(
            "calendar.look",
            r#"{"focus":"work","when":"today","events":[],"schedule":[],"off_days":[]}"#,
        );
        assert!(msg.contains("not working today"));
        assert!(!msg.to_ascii_lowercase().contains("sleep"));
    }

    #[test]
    fn phrases_look_week_lists_times() {
        let msg = phrase_tool_result(
            "calendar.look",
            r#"{"focus":"events","events":[
              {"id":"1","title":"Dentist","start":1786039200000,"end":1786042800000},
              {"id":"2","title":"Climbing","start":1786125600000,"end":1786131000000}
            ]}"#,
        );
        assert!(msg.contains("Dentist"));
        assert!(msg.contains("Climbing"));
        assert!(msg.contains("- Dentist") || msg.contains("- Climbing"));
        assert!(msg.contains("\n- "));
        assert!(!msg.contains("and 0 more"));
    }

    #[test]
    fn phrases_study_sessions_not_events() {
        let msg = phrase_tool_result(
            "study.look",
            r#"[{"date":"2026-08-07","duration_minutes":45,"notes":"chem"},{"date":"2026-08-08","duration_minutes":60}]"#,
        );
        assert!(msg.contains("study session"), "{msg}");
        assert!(msg.contains("45 min"), "{msg}");
        assert!(!msg.to_ascii_lowercase().contains("event"), "{msg}");
    }

    #[test]
    fn phrases_todo_list_as_tasks() {
        let msg = phrase_tool_result(
            "todo.list",
            r#"[{"title":"Buy milk","status":"not_started"},{"title":"Call mum","status":"not_started"}]"#,
        );
        assert!(msg.contains("task"), "{msg}");
        assert!(msg.contains("Buy milk"), "{msg}");
        assert!(!msg.contains("event"), "{msg}");
    }

    #[test]
    fn hides_todo_uuid_in_add_ack() {
        let msg = phrase_tool_result(
            "todo.add",
            "Added todo f245201c-9777-477a-9b68-775e3732acd5 — Buy milk",
        );
        assert!(msg.contains("Buy milk"), "{msg}");
        assert!(!msg.contains("f245201c"), "{msg}");
    }

    #[test]
    fn phrases_sparks_from_content() {
        let msg = phrase_tool_result(
            "list_sparks",
            r#"[{"content":"try cold brew","status":"active"},{"content":"weekend hike","status":"active"}]"#,
        );
        assert!(msg.contains("spark"), "{msg}");
        assert!(msg.contains("cold brew"), "{msg}");
        assert!(!msg.contains("event"), "{msg}");
    }

    #[test]
    fn phrases_save_spark_keeps_content() {
        let msg = phrase_tool_result(
            "save_spark",
            r#"{"id":"s1","content":"climbing tracker app","status":"active"}"#,
        );
        assert!(msg.contains("climbing tracker app"), "{msg}");
        assert!(!msg.contains("event"), "{msg}");
    }

    #[test]
    fn phrases_docs_search_hits() {
        let msg = phrase_tool_result(
            "docs.search",
            r#"[{"title":"study sheet","snippet":"week 3 notes"}]"#,
        );
        assert!(msg.contains("document"), "{msg}");
        assert!(msg.contains("study sheet"), "{msg}");
        assert!(!msg.contains("event"), "{msg}");
    }

    #[test]
    fn phrases_docs_get_content() {
        let msg = phrase_tool_result(
            "docs.get",
            r#"{"id":"1","title":"study sheet","content":"Chapter 1: cells","format":"markdown"}"#,
        );
        assert!(msg.contains("study sheet"), "{msg}");
        assert!(msg.contains("Chapter 1"), "{msg}");
        assert!(!msg.contains("Saved document"), "{msg}");
    }

    #[test]
    fn phrases_money_log_and_food_log() {
        let money = phrase_tool_result(
            "money.log",
            r#"{"kind":"expense","description":"market","amount_cents":1000}"#,
        );
        assert!(money.contains("£10.00"), "{money}");
        assert!(money.contains("market"), "{money}");
        let pot = phrase_tool_result(
            "money.pot",
            r#"{"name":"holiday","amount":200.0,"balance_cents":20000,"mode":"set"}"#,
        );
        assert!(pot.contains("holiday"), "{pot}");
        assert!(pot.contains("£200.00"), "{pot}");
        let food = phrase_tool_result(
            "fitness.log_food",
            r#"{"name":"burrito","calories":750.0,"protein":30.0}"#,
        );
        assert!(food.contains("burrito"), "{food}");
        assert!(food.contains("750"), "{food}");
    }

    #[test]
    fn phrases_study_upsert_and_weight() {
        let topic = phrase_tool_result(
            "study.upsert_topic",
            r#"{"name":"Hamlet","status":"not_started"}"#,
        );
        assert!(topic.contains("Hamlet"), "{topic}");
        assert!(topic.contains("topic"), "{topic}");
        let wt = phrase_tool_result("fitness.log_weight", r#"{"kg":81.2,"date":"2026-08-09"}"#);
        assert!(wt.contains("81.2"), "{wt}");
        let plan = phrase_tool_result(
            "goal.propose_plan",
            r#"{"weekly_minutes_requested":420,"needs_approval":true,"assumptions":["Released time stays free"]}"#,
        );
        assert!(plan.contains("420"), "{plan}");
        assert!(plan.contains("approve"), "{plan}");
    }
}
