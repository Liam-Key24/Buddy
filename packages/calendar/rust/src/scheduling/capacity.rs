use chrono::{Duration, Local, TimeZone};

use crate::models::{Event, ScheduleKind};
use crate::scheduling::occupancy::{build_occupancy, BusySource};
use crate::scheduling::types::DayCapacity;
use crate::scheduling::SchedulingContext;

/// Compute capacity metrics for a local calendar day containing `day_ms`.
pub fn compute_day_capacity(ctx: &SchedulingContext, day_ms: i64) -> DayCapacity {
    let (day_start, day_end) = local_day_bounds(day_ms);
    let date = Local
        .timestamp_millis_opt(day_start)
        .single()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown".into());

    let occupancy = build_occupancy(ctx);

    let sleep_ms: i64 = occupancy
        .iter()
        .filter(|b| b.source == BusySource::Sleep)
        .filter(|b| b.start < day_end && b.end > day_start)
        .map(|b| (b.end.min(day_end) - b.start.max(day_start)).max(0))
        .sum();

    let waking_ms = (86_400_000 - sleep_ms).max(1);
    let waking_hours = waking_ms as f64 / 3_600_000.0;

    let events_in_day: Vec<&Event> = ctx
        .events
        .iter()
        .filter(|e| e.start_time < day_end && e.end_time > day_start && !e.all_day)
        .collect();

    let mut meeting_ms = 0_i64;
    let mut focus_ms = 0_i64;
    for e in &events_in_day {
        let ms = (e.end_time.min(day_end) - e.start_time.max(day_start)).max(0);
        if is_meeting(e) {
            meeting_ms += ms;
        } else {
            focus_ms += ms;
        }
    }

    // Work blocks count as booked (protected) but not as meetings/focus events.
    let work_ms: i64 = ctx
        .lifestyle_blocks
        .iter()
        .filter(|b| b.kind == ScheduleKind::Work)
        .filter(|b| b.start_time < day_end && b.end_time > day_start)
        .map(|b| (b.end_time.min(day_end) - b.start_time.max(day_start)).max(0))
        .sum();

    let booked_ms = meeting_ms + focus_ms;
    let free_ms = (waking_ms - booked_ms - work_ms).max(0);

    let booked_hours = booked_ms as f64 / 3_600_000.0;
    let meeting_hours = meeting_ms as f64 / 3_600_000.0;
    let focus_hours = focus_ms as f64 / 3_600_000.0;
    let free_hours = free_ms as f64 / 3_600_000.0;

    // Overload considers event booked load vs waking hours (work is expected).
    let load_ratio = booked_hours / waking_hours;
    let overloaded = load_ratio >= ctx.policy.overload_threshold || free_hours < 0.5;

    DayCapacity {
        date,
        booked_hours: round2(booked_hours),
        meeting_hours: round2(meeting_hours),
        focus_hours: round2(focus_hours),
        free_hours: round2(free_hours),
        waking_hours: round2(waking_hours),
        overloaded,
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

fn local_day_bounds(ms: i64) -> (i64, i64) {
    let dt = Local
        .timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(Local::now);
    let date = dt.date_naive();
    let start = Local
        .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .unwrap_or_else(|| Local.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()))
        .timestamp_millis();
    (start, start + Duration::days(1).num_milliseconds())
}

/// Public helper for service/summary/planner day windows.
pub fn local_day_bounds_ms(ms: i64) -> (i64, i64) {
    local_day_bounds(ms)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
