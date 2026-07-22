use chrono::{Datelike, Local, NaiveDate, NaiveTime, TimeZone, Timelike, Weekday};
use serde_json::from_str;

use buddy_database::Database;

use crate::error::CalendarError;
use crate::models::{ScheduleBlock, ScheduleKind, ScheduleSegment};

fn weekday_code(d: Weekday) -> &'static str {
    match d {
        Weekday::Mon => "MO",
        Weekday::Tue => "TU",
        Weekday::Wed => "WE",
        Weekday::Thu => "TH",
        Weekday::Fri => "FR",
        Weekday::Sat => "SA",
        Weekday::Sun => "SU",
    }
}

fn parse_hm(hm: &str) -> Result<(u32, u32), CalendarError> {
    let parts: Vec<_> = hm.trim().split(':').collect();
    if parts.len() != 2 {
        return Err(CalendarError::InvalidInput(format!("bad time {hm}")));
    }
    let h: u32 = parts[0]
        .parse()
        .map_err(|_| CalendarError::InvalidInput(format!("bad hour in {hm}")))?;
    let m: u32 = parts[1]
        .parse()
        .map_err(|_| CalendarError::InvalidInput(format!("bad minute in {hm}")))?;
    Ok((h, m))
}

fn local_ms(date: NaiveDate, hour: u32, minute: u32) -> i64 {
    let naive = date
        .and_hms_opt(hour, minute, 0)
        .unwrap_or_else(|| date.and_hms_opt(0, 0, 0).unwrap());
    Local
        .from_local_datetime(&naive)
        .single()
        .unwrap_or_else(|| Local.from_utc_datetime(&naive))
        .timestamp_millis()
}

fn format_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

pub fn parse_date(s: &str) -> Result<NaiveDate, CalendarError> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
        .map_err(|_| CalendarError::InvalidInput(format!("bad date {s}")))
}

/// Expand lifestyle rules into schedule blocks overlapping [start_ms, end_ms).
pub fn list_blocks_in_range(
    db: &Database,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<ScheduleBlock>, CalendarError> {
    let rules = db.list_lifestyle_schedule_rules()?;
    let mut blocks = Vec::new();

    let start_local = Local
        .timestamp_millis_opt(start_ms)
        .single()
        .unwrap_or_else(Local::now);
    let end_local = Local
        .timestamp_millis_opt(end_ms)
        .single()
        .unwrap_or_else(Local::now);

    // Include previous day so overnight sleep that starts before the range still paints.
    let mut day = start_local.date_naive() - chrono::Duration::days(1);
    let last = end_local.date_naive();

    while day <= last {
        let code = weekday_code(day.weekday());
        for rule in &rules {
            let kind = match ScheduleKind::parse(&rule.kind) {
                Some(k) => k,
                None => continue,
            };
            let segments: Vec<ScheduleSegment> = from_str(&rule.segments_json).map_err(|e| {
                CalendarError::InvalidInput(format!("schedule segments: {e}"))
            })?;
            for seg in segments {
                if !seg.by_day.iter().any(|d| d.eq_ignore_ascii_case(code)) {
                    continue;
                }
                let (sh, sm) = parse_hm(&seg.start_hm)?;
                let (eh, em) = parse_hm(&seg.end_hm)?;
                let start = local_ms(day, sh, sm);
                let end = if seg.crosses_midnight {
                    local_ms(day + chrono::Duration::days(1), eh, em)
                } else {
                    local_ms(day, eh, em)
                };
                if end <= start_ms || start >= end_ms {
                    continue;
                }
                let anchor = format_date(day);
                let title = match kind {
                    ScheduleKind::Work => "Work".to_string(),
                    ScheduleKind::Sleep => "Sleep".to_string(),
                };
                blocks.push(ScheduleBlock {
                    id: format!("{}::{anchor}", kind.as_str()),
                    kind,
                    title,
                    start_time: start,
                    end_time: end,
                    anchor_date: anchor,
                });
            }
        }
        day += chrono::Duration::days(1);
    }

    blocks.sort_by_key(|b| b.start_time);
    Ok(blocks)
}

/// Most recent sleep night whose start is at or before `now` (local).
pub fn last_sleep_date(db: &Database, now_ms: Option<i64>) -> Result<String, CalendarError> {
    let now = match now_ms {
        Some(ms) => Local
            .timestamp_millis_opt(ms)
            .single()
            .unwrap_or_else(Local::now),
        None => Local::now(),
    };
    let blocks = list_blocks_in_range(
        db,
        (now - chrono::Duration::days(3)).timestamp_millis(),
        (now + chrono::Duration::hours(1)).timestamp_millis(),
    )?;
    let sleep = blocks
        .into_iter()
        .filter(|b| b.kind == ScheduleKind::Sleep && b.start_time <= now.timestamp_millis())
        .max_by_key(|b| b.start_time);
    match sleep {
        Some(b) => Ok(b.anchor_date),
        None => {
            // Fallback: if before evening, yesterday; else today.
            let date = if now.time() < NaiveTime::from_hms_opt(12, 0, 0).unwrap() {
                now.date_naive() - chrono::Duration::days(1)
            } else {
                now.date_naive()
            };
            Ok(format_date(date))
        }
    }
}

/// Template work hours for a weekday date (ms), if work is scheduled that day.
pub fn template_work_bounds(
    db: &Database,
    work_date: &str,
) -> Result<Option<(i64, i64)>, CalendarError> {
    let date = parse_date(work_date)?;
    let code = weekday_code(date.weekday());
    let rule = match db.get_lifestyle_schedule_rule("work") {
        Ok(r) => r,
        Err(buddy_database::DbError::NotFound(_)) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let segments: Vec<ScheduleSegment> =
        from_str(&rule.segments_json).map_err(|e| CalendarError::InvalidInput(e.to_string()))?;
    for seg in segments {
        if !seg.by_day.iter().any(|d| d.eq_ignore_ascii_case(code)) {
            continue;
        }
        let (sh, sm) = parse_hm(&seg.start_hm)?;
        let (eh, em) = parse_hm(&seg.end_hm)?;
        let start = local_ms(date, sh, sm);
        let end = if seg.crosses_midnight {
            local_ms(date + chrono::Duration::days(1), eh, em)
        } else {
            local_ms(date, eh, em)
        };
        return Ok(Some((start, end)));
    }
    Ok(None)
}

pub fn hours_between(start_ms: i64, end_ms: i64) -> f64 {
    ((end_ms - start_ms).max(0) as f64) / 3_600_000.0
}

/// Local YYYY-MM-DD for a timestamp.
#[allow(dead_code)]
pub fn local_date_string(ms: i64) -> String {
    let dt = Local
        .timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(Local::now);
    format_date(dt.date_naive())
}

pub fn today_date_string() -> String {
    format_date(Local::now().date_naive())
}

pub fn set_time_on_date(work_date: &str, hour: u32, minute: u32) -> Result<i64, CalendarError> {
    let date = parse_date(work_date)?;
    Ok(local_ms(date, hour, minute))
}

#[allow(dead_code)]
pub fn local_now_hm() -> (u32, u32) {
    let n = Local::now();
    (n.hour(), n.minute())
}
