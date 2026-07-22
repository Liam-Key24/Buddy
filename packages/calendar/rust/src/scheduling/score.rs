use chrono::{Local, Timelike, TimeZone};

use crate::models::{EventPriority, Flexibility};
use crate::scheduling::occupancy::{BusyInterval, BusySource};
use crate::scheduling::SchedulingContext;

/// Score a candidate slot. Higher is better. Never prefers earliest alone.
pub fn score_slot(
    ctx: &SchedulingContext,
    start: i64,
    end: i64,
    occupancy: &[BusyInterval],
) -> (f64, Vec<String>) {
    score_slot_for_activity(ctx, start, end, occupancy, None)
}

/// Score a slot, optionally biasing toward natural hours for an activity title.
pub fn score_slot_for_activity(
    ctx: &SchedulingContext,
    start: i64,
    end: i64,
    occupancy: &[BusyInterval],
    activity_title: Option<&str>,
) -> (f64, Vec<String>) {
    let mut score = 100.0_f64;
    let mut reasons = Vec::new();

    for b in occupancy {
        if !b.overlaps(start, end) {
            continue;
        }
        match b.source {
            BusySource::Sleep => {
                score -= 10_000.0;
                reasons.push("overlaps sleep".into());
            }
            BusySource::Work => {
                score -= 10_000.0;
                reasons.push("overlaps work block".into());
            }
            BusySource::Event => {
                score -= 5_000.0;
                reasons.push(format!(
                    "overlaps {}",
                    b.label.as_deref().unwrap_or("event")
                ));
            }
            BusySource::Buffer => {
                if !ctx.policy.allow_reduce_buffer {
                    score -= 2_000.0;
                    reasons.push("violates buffer".into());
                } else {
                    score -= 50.0;
                    reasons.push("uses reduced buffer".into());
                }
            }
        }
    }

    let duration_ms = (end - start).max(1) as f64;
    let day_start = day_bounds(start).0;
    let day_end = day_bounds(start).1;

    let contiguous = contiguous_free_around(start, end, occupancy, day_start, day_end);
    let contiguous_hours = contiguous as f64 / 3_600_000.0;
    score += (contiguous_hours * 8.0).min(40.0);
    if contiguous_hours >= 2.0 {
        reasons.push("strong contiguous focus window".into());
    }

    let meeting_ms = occupancy
        .iter()
        .filter(|b| b.source == BusySource::Event)
        .filter(|b| b.start < day_end && b.end > day_start)
        .map(|b| (b.end.min(day_end) - b.start.max(day_start)).max(0))
        .sum::<i64>() as f64;
    let meeting_hours = meeting_ms / 3_600_000.0;
    score -= meeting_hours * 6.0;
    if meeting_hours > 4.0 {
        reasons.push("busy meeting day".into());
    } else if meeting_hours < 1.0 {
        reasons.push("light meeting day".into());
        score += 10.0;
    }

    let hour = Local
        .timestamp_millis_opt(start)
        .single()
        .map(|dt| dt.hour())
        .unwrap_or(12);

    let focus_hour_bonus = match hour {
        9..=11 => 15.0,
        13..=16 => 12.0,
        8 | 12 | 17 => 5.0,
        _ => -5.0,
    };
    score += focus_hour_bonus;
    if (9..=11).contains(&hour) || (13..=16).contains(&hour) {
        reasons.push("preferred focus period".into());
    }

    if let Some(title) = activity_title {
        let (adj, reason) = activity_hour_adjustment(title, hour);
        score += adj;
        if let Some(r) = reason {
            reasons.push(r.into());
        }
    }

    let offset_from_day = (start - day_start) as f64 / 3_600_000.0;
    score -= (offset_from_day * 0.3).min(5.0);

    let residual = contiguous as f64 - duration_ms;
    if residual >= 30.0 * 60_000.0 {
        score += 8.0;
        reasons.push("leaves residual focus time".into());
    } else if residual < 15.0 * 60_000.0 {
        score -= 6.0;
        reasons.push("fills gap tightly".into());
    }

    let _ = EventPriority::Normal;
    let _ = Flexibility::Fixed;

    (score, reasons)
}

/// Bias slots toward natural hours for the activity (dinner ≠ 7:45am).
pub fn activity_hour_adjustment(title: &str, hour: u32) -> (f64, Option<&'static str>) {
    let t = title.to_ascii_lowercase();
    if is_dinner_like(&t) {
        return match hour {
            17..=20 => (100.0, Some("evening meal window")),
            16 | 21 => (50.0, Some("near dinner time")),
            0..=11 => (-150.0, Some("too early for dinner")),
            12..=15 => (-40.0, None),
            _ => (-10.0, None),
        };
    }
    if is_lunch_like(&t) {
        return match hour {
            11..=14 => (80.0, Some("lunch window")),
            0..=9 | 18..=23 => (-80.0, Some("outside lunch hours")),
            _ => (0.0, None),
        };
    }
    if is_breakfast_like(&t) {
        return match hour {
            6..=10 => (80.0, Some("breakfast window")),
            11..=23 => (-80.0, Some("outside breakfast hours")),
            _ => (0.0, None),
        };
    }
    if t.contains("bath") || t.contains("shower") {
        return match hour {
            18..=22 => (35.0, Some("evening wind-down")),
            6..=9 => (15.0, None),
            _ => (0.0, None),
        };
    }
    if t.contains("tennis")
        || t.contains("gym")
        || t.contains("run")
        || t.contains("workout")
        || t.contains("sport")
    {
        return match hour {
            16..=20 => (40.0, Some("after-work activity")),
            6..=9 => (25.0, Some("morning activity")),
            10..=15 => (-30.0, None),
            _ => (0.0, None),
        };
    }
    (0.0, None)
}

fn is_dinner_like(t: &str) -> bool {
    t.contains("dinner")
        || t.contains("supper")
        || t.contains("cooking dinner")
        || (t.contains("cooking") && !t.contains("breakfast") && !t.contains("lunch"))
        || (t.contains("cook")
            && (t.contains("dinner") || t.contains("evening") || t.contains("meal")))
}

fn is_lunch_like(t: &str) -> bool {
    t.contains("lunch") || t.contains("brunch")
}

fn is_breakfast_like(t: &str) -> bool {
    t.contains("breakfast")
}

fn day_bounds(ms: i64) -> (i64, i64) {
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
    (start, start + 86_400_000)
}

fn contiguous_free_around(
    start: i64,
    end: i64,
    occupancy: &[BusyInterval],
    day_start: i64,
    day_end: i64,
) -> i64 {
    let hard: Vec<(i64, i64)> = occupancy
        .iter()
        .filter(|b| b.source != BusySource::Buffer)
        .map(|b| (b.start, b.end))
        .collect();

    let prev = hard
        .iter()
        .filter(|(s, e)| *e <= start && *s >= day_start)
        .map(|(_, e)| *e)
        .max()
        .unwrap_or(day_start);
    let next = hard
        .iter()
        .filter(|(s, _)| *s >= end && *s < day_end)
        .map(|(s, _)| *s)
        .min()
        .unwrap_or(day_end);
    (next - prev).max(end - start)
}
