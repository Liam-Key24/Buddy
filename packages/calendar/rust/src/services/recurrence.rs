use chrono::{Datelike, Duration, TimeZone, Timelike, Utc};

use crate::models::{Event, RecurrenceRule};

/// Expand a recurring master event into occurrences that overlap `[range_start, range_end)`.
/// Non-recurring events are returned as-is if they overlap the range.
pub fn expand_event_in_range(event: &Event, range_start: i64, range_end: i64) -> Vec<Event> {
    let Some(rule) = &event.recurrence else {
        if event.start_time < range_end && event.end_time > range_start {
            return vec![event.clone()];
        }
        return Vec::new();
    };

    let duration = (event.end_time - event.start_time).max(0);
    let interval = rule.interval.max(1) as i64;
    let freq = rule.frequency.to_ascii_uppercase();

    let mut occurrences = Vec::new();
    let mut cursor = event.start_time;
    let mut count = 0u32;
    let max_iterations = 500;

    for _ in 0..max_iterations {
        if let Some(until) = rule.until {
            if cursor > until {
                break;
            }
        }
        if let Some(max_count) = rule.count {
            if count >= max_count {
                break;
            }
        }

        let occ_end = cursor + duration;
        if cursor < range_end && occ_end > range_start {
            let mut occ = event.clone();
            occ.start_time = cursor;
            occ.end_time = occ_end;
            if count > 0 {
                occ.id = format!("{}::{}", event.id, cursor);
                occ.occurrence_of = Some(event.id.clone());
            }
            occurrences.push(occ);
        }

        count += 1;
        let next = match freq.as_str() {
            "DAILY" => cursor + Duration::days(interval).num_milliseconds(),
            "WEEKLY" => advance_weekly(cursor, interval, &rule.by_day),
            "MONTHLY" => advance_monthly(cursor, interval),
            "YEARLY" => advance_yearly(cursor, interval),
            _ => break,
        };

        if next <= cursor {
            break;
        }
        // Stop if we've gone well past the range with no more useful occurrences
        if cursor > range_end && count > 0 {
            break;
        }
        cursor = next;
    }

    occurrences
}

fn advance_weekly(cursor: i64, interval: i64, by_day: &[String]) -> i64 {
    if by_day.is_empty() {
        return cursor + Duration::weeks(interval).num_milliseconds();
    }
    // Simple: jump by one day until we land on a matching weekday, wrapping weeks by interval.
    let mut next = cursor + Duration::days(1).num_milliseconds();
    for _ in 0..21 {
        let dt = Utc.timestamp_millis_opt(next).single();
        if let Some(dt) = dt {
            let wd = weekday_code(dt.weekday().num_days_from_sunday());
            if by_day.iter().any(|d| d.eq_ignore_ascii_case(&wd)) {
                // If we've crossed into a new interval week from the original, ok.
                return next;
            }
        }
        next += Duration::days(1).num_milliseconds();
    }
    cursor + Duration::weeks(interval).num_milliseconds()
}

fn weekday_code(num_from_sunday: u32) -> String {
    match num_from_sunday {
        0 => "SU".into(),
        1 => "MO".into(),
        2 => "TU".into(),
        3 => "WE".into(),
        4 => "TH".into(),
        5 => "FR".into(),
        _ => "SA".into(),
    }
}

fn advance_monthly(cursor: i64, interval: i64) -> i64 {
    let Some(dt) = Utc.timestamp_millis_opt(cursor).single() else {
        return cursor + Duration::days(30 * interval).num_milliseconds();
    };
    let mut year = dt.year();
    let mut month = dt.month() as i32 + interval as i32;
    while month > 12 {
        month -= 12;
        year += 1;
    }
    let day = dt.day().min(days_in_month(year, month as u32));
    match Utc.with_ymd_and_hms(year, month as u32, day, dt.hour(), dt.minute(), dt.second()) {
        chrono::LocalResult::Single(next) => next.timestamp_millis(),
        _ => cursor + Duration::days(30 * interval).num_milliseconds(),
    }
}

fn advance_yearly(cursor: i64, interval: i64) -> i64 {
    let Some(dt) = Utc.timestamp_millis_opt(cursor).single() else {
        return cursor + Duration::days(365 * interval).num_milliseconds();
    };
    let year = dt.year() + interval as i32;
    let day = dt.day().min(days_in_month(year, dt.month()));
    match Utc.with_ymd_and_hms(year, dt.month(), day, dt.hour(), dt.minute(), dt.second()) {
        chrono::LocalResult::Single(next) => next.timestamp_millis(),
        _ => cursor + Duration::days(365 * interval).num_milliseconds(),
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Parse recurrence from optional JSON string.
pub fn parse_recurrence(json: &Option<String>) -> Option<RecurrenceRule> {
    json.as_ref()
        .and_then(|s| serde_json::from_str::<RecurrenceRule>(s).ok())
}

pub fn serialize_recurrence(rule: &Option<RecurrenceRule>) -> Option<String> {
    rule.as_ref()
        .and_then(|r| serde_json::to_string(r).ok())
}
