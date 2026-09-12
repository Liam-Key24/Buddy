//! Compact calendar snapshot for look / occupancy (events + lifestyle + off days).

use chrono::{Duration, Local, TimeZone};
use serde::{Deserialize, Serialize};

use crate::models::{Event, EventPriority, Flexibility, ScheduleBlock, ScheduleKind};
use crate::scheduling::types::FreeSlot;
use crate::scheduling::SchedulingContext;

/// Timed event as returned by `calendar.look`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookEvent {
    pub id: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub all_day: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub category: String,
    pub priority: EventPriority,
    pub flexibility: Flexibility,
}

/// Work / sleep layer as returned by `calendar.look`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookScheduleBlock {
    pub id: String,
    pub kind: ScheduleKind,
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub anchor_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookSlot {
    pub start: i64,
    pub end: i64,
}

/// Full picture: events, lifestyle hours, holidays, optional free slots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookSnapshot {
    pub focus: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    pub events: Vec<LookEvent>,
    pub schedule: Vec<LookScheduleBlock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub off_days: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<LookSlot>,
}

/// Holiday / leave: work template is off; not a midnight–midnight busy block.
pub fn is_off_day_event(event: &Event) -> bool {
    if event.category.eq_ignore_ascii_case("holidays") {
        return true;
    }
    let t = event.title.to_ascii_lowercase();
    t.contains("holiday")
        || t.contains("vacation")
        || t.contains("annual leave")
        || t.contains("bank holiday")
        || t.contains("day off")
        || t.split_whitespace().any(|w| w == "pto")
}

/// All-day markers (holiday, birthday) are not timed occupancy.
pub fn event_occupies_time(event: &Event) -> bool {
    !event.all_day
}

pub fn local_date_string(ms: i64) -> String {
    Local
        .timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(Local::now)
        .format("%Y-%m-%d")
        .to_string()
}

/// Local YYYY-MM-DD dates covered by `[start, end)`.
pub fn dates_spanned(start_ms: i64, end_ms: i64) -> Vec<String> {
    if end_ms <= start_ms {
        return vec![local_date_string(start_ms)];
    }
    let last = end_ms.saturating_sub(1);
    let mut day_start = crate::scheduling::local_day_bounds_ms(start_ms).0;
    let last_start = crate::scheduling::local_day_bounds_ms(last).0;
    let mut out = Vec::new();
    while day_start <= last_start {
        out.push(local_date_string(day_start));
        day_start += Duration::days(1).num_milliseconds();
        if out.len() > 62 {
            break;
        }
    }
    out
}

pub fn off_day_dates(events: &[Event]) -> Vec<String> {
    let mut dates = Vec::new();
    for event in events {
        if !is_off_day_event(event) {
            continue;
        }
        for d in dates_spanned(event.start_time, event.end_time) {
            if !dates.iter().any(|x| x == &d) {
                dates.push(d);
            }
        }
    }
    dates.sort();
    dates
}

/// Drop Work blocks on holiday / day-off dates. Sleep is unchanged.
pub fn punch_work_for_off_days(
    events: &[Event],
    blocks: Vec<ScheduleBlock>,
) -> Vec<ScheduleBlock> {
    let off = off_day_dates(events);
    if off.is_empty() {
        return blocks;
    }
    blocks
        .into_iter()
        .filter(|block| {
            if block.kind != ScheduleKind::Work {
                return true;
            }
            if off.iter().any(|d| d == &block.anchor_date) {
                return false;
            }
            !dates_spanned(block.start_time, block.end_time)
                .iter()
                .any(|d| off.iter().any(|o| o == d))
        })
        .collect()
}

pub fn compose_look_snapshot(
    ctx: &SchedulingContext,
    when: Option<String>,
    focus: &str,
    slots: Vec<FreeSlot>,
    query: Option<&str>,
) -> LookSnapshot {
    let q = query
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty());
    let mut events: Vec<LookEvent> = ctx
        .events
        .iter()
        .filter(|e| {
            q.as_ref()
                .map(|ql| e.title.to_ascii_lowercase().contains(ql.as_str()))
                .unwrap_or(true)
        })
        .map(look_event)
        .collect();
    events.sort_by_key(|e| (e.start, e.end));

    let mut schedule: Vec<LookScheduleBlock> = ctx.lifestyle_blocks.iter().map(look_block).collect();
    schedule.sort_by_key(|b| (b.start, b.end));

    LookSnapshot {
        focus: focus.to_string(),
        when,
        events,
        schedule,
        off_days: off_day_dates(&ctx.events),
        slots: slots
            .into_iter()
            .map(|s| LookSlot {
                start: s.start,
                end: s.end,
            })
            .collect(),
    }
}

fn look_event(event: &Event) -> LookEvent {
    LookEvent {
        id: event.id.clone(),
        title: event.title.clone(),
        start: event.start_time,
        end: event.end_time,
        all_day: event.all_day,
        location: event.location.clone(),
        category: event.category.clone(),
        priority: event.priority,
        flexibility: event.flexibility,
    }
}

pub(crate) fn look_block(block: &ScheduleBlock) -> LookScheduleBlock {
    LookScheduleBlock {
        id: block.id.clone(),
        kind: block.kind,
        title: block.title.clone(),
        start: block.start_time,
        end: block.end_time,
        anchor_date: block.anchor_date.clone(),
    }
}
