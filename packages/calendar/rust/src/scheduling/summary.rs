use serde::{Deserialize, Serialize};

use crate::models::Event;
use crate::scheduling::capacity::compute_day_capacity;
use crate::scheduling::conflict::detect_conflicts;
use crate::scheduling::free_time::find_free_slots;
use crate::scheduling::occupancy::{build_occupancy, BusySource};
use crate::scheduling::types::{DayCapacity, Suggestion, SuggestionAction};
use crate::scheduling::SchedulingContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaySummary {
    pub date: String,
    pub capacity: DayCapacity,
    pub meetings: Vec<SummaryItem>,
    pub focus_blocks: Vec<SummaryItem>,
    pub free_slots: Vec<SummaryItem>,
    pub conflicts: Vec<String>,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryItem {
    pub title: String,
    pub start: i64,
    pub end: i64,
}

pub fn compose_day_summary(ctx: &SchedulingContext, day_ms: i64) -> DaySummary {
    use chrono::{Timelike, TimeZone, Local};
    let (day_start, day_end) = crate::scheduling::local_day_bounds_ms(day_ms);

    let mut day_ctx = ctx.clone();
    day_ctx.range.start = day_start;
    day_ctx.range.end = day_end;

    let capacity = compute_day_capacity(&day_ctx, day_ms);
    let events: Vec<&Event> = day_ctx
        .events
        .iter()
        .filter(|e| e.start_time < day_end && e.end_time > day_start)
        .collect();

    let mut meetings = Vec::new();
    let mut focus_blocks = Vec::new();
    for e in &events {
        let item = SummaryItem {
            title: e.title.clone(),
            start: e.start_time,
            end: e.end_time,
        };
        if is_meeting(e) {
            meetings.push(item);
        } else {
            focus_blocks.push(item);
        }
    }

    let free = find_free_slots(&day_ctx, 30 * 60_000, 8, None);
    let free_slots: Vec<SummaryItem> = free
        .iter()
        .map(|s| SummaryItem {
            title: format!("Free ({:.0} min)", (s.end - s.start) as f64 / 60_000.0),
            start: s.start,
            end: s.end,
        })
        .collect();

    let mut conflicts = Vec::new();
    for e in &events {
        let report = detect_conflicts(&day_ctx, e.start_time, e.end_time, Some(&e.id));
        for c in report.conflicts {
            conflicts.push(format!("{}: {}", e.title, c.message));
        }
    }

    let mut suggestions = Vec::new();
    if capacity.overloaded {
        suggestions.push(Suggestion {
            action: SuggestionAction::Redistribute,
            message: "Day is overloaded — move flexible events to another day.".into(),
            event_id: None,
            start: None,
            end: None,
        });
    }

    let occupancy = build_occupancy(&day_ctx);
    let meeting_count = occupancy
        .iter()
        .filter(|b| b.source == BusySource::Event)
        .filter(|b| {
            meetings
                .iter()
                .any(|m| Some(m.title.as_str()) == b.label.as_deref())
        })
        .count()
        .max(meetings.len());

    let has_long_focus = find_free_slots(&day_ctx, 90 * 60_000, 1, None);
    if meeting_count >= 3 && has_long_focus.is_empty() && focus_blocks.is_empty() {
        suggestions.push(Suggestion {
            action: SuggestionAction::ProtectFocus,
            message: "No uninterrupted focus time — consider protecting an afternoon focus block."
                .into(),
            event_id: None,
            start: None,
            end: None,
        });
    }

    // Lunch gap suggestion around midday if fully booked.
    let midday_free = free.iter().any(|s| {
        let mid = (s.start + s.end) / 2;
        let hour = Local
            .timestamp_millis_opt(mid)
            .single()
            .map(|d| d.hour())
            .unwrap_or(0);
        (11..=14).contains(&hour) && (s.end - s.start) >= 30 * 60_000
    });
    if !midday_free && !events.is_empty() {
        suggestions.push(Suggestion {
            action: SuggestionAction::AddBreak,
            message: "Add a lunch break around midday.".into(),
            event_id: None,
            start: None,
            end: None,
        });
    }

    // Suggest moving a flexible meeting if overloaded.
    if capacity.overloaded {
        if let Some(flex) = events.iter().find(|e| e.flexibility.is_movable()) {
            suggestions.push(Suggestion {
                action: SuggestionAction::MoveNew,
                message: format!("Move \"{}\" to free capacity.", flex.title),
                event_id: Some(flex.id.clone()),
                start: None,
                end: None,
            });
        }
    }

    DaySummary {
        date: capacity.date.clone(),
        capacity,
        meetings,
        focus_blocks,
        free_slots,
        conflicts,
        suggestions,
    }
}

fn is_meeting(event: &Event) -> bool {
    let t = event.title.to_ascii_lowercase();
    event.category == "work"
        || t.contains("meeting")
        || t.contains("call")
        || t.contains("standup")
        || t.contains("sync")
}
