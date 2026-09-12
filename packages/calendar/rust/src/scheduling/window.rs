//! Natural-language windows, "when" constraints, and local datetimes for AI tools.

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone, Weekday};

use crate::models::DateRange;

/// Resolve a look/organize window label into a local `[start, end)` range.
pub fn resolve_window(label: Option<&str>) -> DateRange {
    let now = Local::now();
    let raw = label.unwrap_or("this_week").trim().to_ascii_lowercase();
    match raw.as_str() {
        "" | "this_week" | "week" | "this week" => local_week_range(now),
        "next_week" | "next week" => local_next_week_range(now),
        "not_today" | "not today" | "from_tomorrow" => local_not_today_range(now),
        "today" | "tonight" => local_day_range(now.date_naive()),
        "tomorrow" => local_day_range(now.date_naive() + Duration::days(1)),
        "weekend" => local_weekend_range(now),
        "sunday" | "on sunday" => named_weekday_range(now, Weekday::Sun),
        "monday" | "on monday" => named_weekday_range(now, Weekday::Mon),
        "tuesday" | "on tuesday" => named_weekday_range(now, Weekday::Tue),
        "wednesday" | "on wednesday" => named_weekday_range(now, Weekday::Wed),
        "thursday" | "on thursday" => named_weekday_range(now, Weekday::Thu),
        "friday" | "on friday" => named_weekday_range(now, Weekday::Fri),
        "saturday" | "on saturday" => named_weekday_range(now, Weekday::Sat),
        other => parse_date_only(other)
            .map(local_day_range)
            .unwrap_or_else(|| local_week_range(now)),
    }
}

/// Extract a canonical when/window label from free text (`next_week`, `friday`, …).
pub fn parse_when_label(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("next week") || lower.contains("next_week") {
        return Some("next_week");
    }
    if lower.contains("weekend") {
        return Some("weekend");
    }
    if lower.contains("this week")
        || lower.contains("this_week")
        || (lower.contains("the week") && !lower.contains("weekend"))
        || lower.contains("all week")
        || lower.contains("whole week")
        || lower.contains("entire week")
    {
        return Some("this_week");
    }
    if lower.contains("tomorrow") {
        return Some("tomorrow");
    }
    if negates_today(&lower) {
        return Some("not_today");
    }
    if lower.contains("today") || lower.contains("tonight") {
        return Some("today");
    }
    for (needle, label) in [
        ("monday", "monday"),
        ("tuesday", "tuesday"),
        ("wednesday", "wednesday"),
        ("thursday", "thursday"),
        ("friday", "friday"),
        ("saturday", "saturday"),
        ("sunday", "sunday"),
    ] {
        if lower.contains(needle) {
            return Some(label);
        }
    }
    None
}

/// Same as [`resolve_window`] but treats empty as today (for look).
pub fn resolve_when_range(when: Option<&str>) -> DateRange {
    match when.map(str::trim).filter(|s| !s.is_empty()) {
        None => resolve_window(Some("today")),
        Some(s) => resolve_window(Some(s)),
    }
}

/// Parse a local wall time: ISO, "tomorrow 14:00", "Friday 10am", unix ms.
pub fn parse_local_datetime(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(n) = trimmed.parse::<i64>() {
        if n > 1_000_000_000_000 {
            return Some(n);
        }
        if n > 1_000_000_000 {
            return Some(n * 1000);
        }
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        return Some(dt.timestamp_millis());
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"]
    {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(trimmed, fmt) {
            return local_naive_ms(naive.date(), naive.time());
        }
    }
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return local_naive_ms(date, NaiveTime::from_hms_opt(9, 0, 0)?);
    }

    let lower = trimmed.to_ascii_lowercase();
    let (day_part, time_part) = split_day_and_time(&lower)?;
    let date = relative_date(&day_part)?;
    let time = parse_clock(&time_part)?;
    local_naive_ms(date, time)
}

/// Parse duration like "90m", "1:30hr", "2 hours", "30min", or a bare number (minutes).
pub fn parse_duration_minutes(raw: &str) -> Option<u32> {
    let s = raw.trim().to_ascii_lowercase().replace(' ', "");
    if s.is_empty() {
        return None;
    }
    if let Ok(n) = s.parse::<u32>() {
        return Some(n.clamp(15, 8 * 60));
    }
    if let Some(caps) = regex_hms(&s) {
        return Some(caps.clamp(15, 8 * 60));
    }
    None
}

fn regex_hms(s: &str) -> Option<u32> {
    // 1:30hr / 1.5h / 90m / 2hours / 30min
    if let Some(rest) = s.strip_suffix("hours").or_else(|| s.strip_suffix("hour")) {
        if let Ok(h) = rest.parse::<f64>() {
            return Some((h * 60.0) as u32);
        }
    }
    if let Some(rest) = s.strip_suffix('h') {
        if rest.contains(':') {
            let mut parts = rest.split(':');
            let h: u32 = parts.next()?.parse().ok()?;
            let m: u32 = parts.next()?.parse().ok()?;
            return Some(h * 60 + m);
        }
        if let Ok(h) = rest.parse::<f64>() {
            return Some((h * 60.0) as u32);
        }
    }
    if let Some(rest) = s
        .strip_suffix("mins")
        .or_else(|| s.strip_suffix("min"))
        .or_else(|| s.strip_suffix('m'))
    {
        return rest.parse::<u32>().ok();
    }
    if s.contains(':') && s.ends_with("hr") {
        let rest = s.trim_end_matches("hr");
        let mut parts = rest.split(':');
        let h: u32 = parts.next()?.parse().ok()?;
        let m: u32 = parts.next()?.parse().ok()?;
        return Some(h * 60 + m);
    }
    None
}

/// Constraint tokens inferred from free text (after_work, weekend, morning, …).
pub fn infer_constraint_tokens(text: &str) -> Vec<String> {
    let t = text.to_ascii_lowercase();
    let mut out = Vec::new();
    if t.contains("after work") || t.contains("after_work") {
        out.push("after_work".into());
    }
    if t.contains("weekend") {
        out.push("weekend".into());
    }
    if t.contains("morning") {
        out.push("morning".into());
    }
    if t.contains("evening") {
        out.push("evening".into());
    }
    out
}

pub fn constraint_after_work(constraints: &[String], when: Option<&str>) -> bool {
    constraints.iter().any(|c| c.eq_ignore_ascii_case("after_work"))
        || when.is_some_and(|w| w.eq_ignore_ascii_case("after_work") || w.contains("after work"))
}

pub fn constraint_weekend(constraints: &[String], when: Option<&str>) -> bool {
    constraints.iter().any(|c| c.eq_ignore_ascii_case("weekend"))
        || when.is_some_and(|w| w.eq_ignore_ascii_case("weekend"))
}

fn local_day_range(date: NaiveDate) -> DateRange {
    let start = local_naive_ms(date, NaiveTime::from_hms_opt(0, 0, 0).unwrap()).unwrap_or(0);
    DateRange {
        start,
        end: start + 86_400_000,
    }
}

fn local_week_range(now: chrono::DateTime<Local>) -> DateRange {
    let today = now.date_naive();
    let weekday = today.weekday().num_days_from_monday() as i64;
    let monday = today - Duration::days(weekday);
    let start = local_naive_ms(monday, NaiveTime::from_hms_opt(0, 0, 0).unwrap()).unwrap_or(0);
    DateRange {
        start,
        end: start + 7 * 86_400_000,
    }
}

fn local_next_week_range(now: chrono::DateTime<Local>) -> DateRange {
    let this = local_week_range(now);
    DateRange {
        start: this.end,
        end: this.end + 7 * 86_400_000,
    }
}

/// Tomorrow through the end of this week (or next week if today is Sunday).
fn local_not_today_range(now: chrono::DateTime<Local>) -> DateRange {
    let tomorrow = now.date_naive() + Duration::days(1);
    let tomorrow_start = local_day_range(tomorrow).start;
    let this = local_week_range(now);
    if tomorrow_start < this.end {
        DateRange {
            start: tomorrow_start,
            end: this.end,
        }
    } else {
        local_next_week_range(now)
    }
}

fn negates_today(lower: &str) -> bool {
    lower.contains("not today")
        || lower.contains("not tonight")
        || lower.contains("n't today")
        || lower.contains("except today")
}

fn local_weekend_range(now: chrono::DateTime<Local>) -> DateRange {
    let today = now.date_naive();
    let sat = match today.weekday() {
        Weekday::Sat => today,
        Weekday::Sun => today - Duration::days(1),
        other => {
            let from_mon = other.num_days_from_monday() as i64;
            today + Duration::days(5 - from_mon)
        }
    };
    let start = local_naive_ms(sat, NaiveTime::from_hms_opt(0, 0, 0).unwrap()).unwrap_or(0);
    DateRange {
        start,
        end: start + 2 * 86_400_000,
    }
}

fn named_weekday_range(now: chrono::DateTime<Local>, target: Weekday) -> DateRange {
    let today = now.date_naive();
    let today_n = today.weekday().num_days_from_monday() as i64;
    let target_n = target.num_days_from_monday() as i64;
    let mut delta = target_n - today_n;
    if delta < 0 {
        delta += 7;
    }
    local_day_range(today + Duration::days(delta))
}

fn parse_date_only(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

fn local_naive_ms(date: NaiveDate, time: NaiveTime) -> Option<i64> {
    let naive = date.and_time(time);
    Local
        .from_local_datetime(&naive)
        .single()
        .or_else(|| Local.from_local_datetime(&naive).earliest())
        .map(|dt| dt.timestamp_millis())
}

fn split_day_and_time(lower: &str) -> Option<(String, String)> {
    if let Some(rest) = lower.strip_prefix("tomorrow ") {
        return Some(("tomorrow".into(), rest.trim().into()));
    }
    if let Some(rest) = lower.strip_prefix("today ") {
        return Some(("today".into(), rest.trim().into()));
    }
    for day in [
        "monday",
        "tuesday",
        "wednesday",
        "thursday",
        "friday",
        "saturday",
        "sunday",
    ] {
        if let Some(rest) = lower.strip_prefix(&format!("{day} ")) {
            return Some((day.into(), rest.trim().into()));
        }
        if let Some(rest) = lower.strip_prefix(&format!("on {day} ")) {
            return Some((day.into(), rest.trim().into()));
        }
    }
    // "2pm" / "14:00" today
    if parse_clock(lower).is_some() {
        return Some(("today".into(), lower.into()));
    }
    None
}

fn relative_date(day_part: &str) -> Option<NaiveDate> {
    let now = Local::now().date_naive();
    match day_part {
        "today" => Some(now),
        "tomorrow" => Some(now + Duration::days(1)),
        "monday" => Some(weekday_date(Weekday::Mon)),
        "tuesday" => Some(weekday_date(Weekday::Tue)),
        "wednesday" => Some(weekday_date(Weekday::Wed)),
        "thursday" => Some(weekday_date(Weekday::Thu)),
        "friday" => Some(weekday_date(Weekday::Fri)),
        "saturday" => Some(weekday_date(Weekday::Sat)),
        "sunday" => Some(weekday_date(Weekday::Sun)),
        _ => None,
    }
}

fn weekday_date(target: Weekday) -> NaiveDate {
    let today = Local::now().date_naive();
    let today_n = today.weekday().num_days_from_monday() as i64;
    let target_n = target.num_days_from_monday() as i64;
    let mut delta = target_n - today_n;
    if delta < 0 {
        delta += 7;
    }
    today + Duration::days(delta)
}

fn parse_clock(raw: &str) -> Option<NaiveTime> {
    let s = raw.trim().to_ascii_lowercase().replace(' ', "");
    let s = s.replace("a.m.", "am").replace("p.m.", "pm");
    if let Ok(t) = NaiveTime::parse_from_str(&s, "%H:%M") {
        return Some(t);
    }
    if let Ok(t) = NaiveTime::parse_from_str(&s, "%H:%M:%S") {
        return Some(t);
    }
    let (digits, pm) = if let Some(rest) = s.strip_suffix("pm") {
        (rest, Some(true))
    } else if let Some(rest) = s.strip_suffix("am") {
        (rest, Some(false))
    } else {
        (s.as_str(), None)
    };
    if digits.is_empty() {
        return None;
    }
    let (h, m) = if let Some((hh, mm)) = digits.split_once(':') {
        (hh.parse::<u32>().ok()?, mm.parse::<u32>().ok()?)
    } else {
        (digits.parse::<u32>().ok()?, 0)
    };
    let hour = match pm {
        Some(true) if h < 12 => h + 12,
        Some(false) if h == 12 => 0,
        _ => h,
    };
    NaiveTime::from_hms_opt(hour, m, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn duration_variants() {
        assert_eq!(parse_duration_minutes("90"), Some(90));
        assert_eq!(parse_duration_minutes("90m"), Some(90));
        assert_eq!(parse_duration_minutes("1:30hr"), Some(90));
        assert_eq!(parse_duration_minutes("2 hours"), Some(120));
    }

    #[test]
    fn tomorrow_clock_parses() {
        let ms = parse_local_datetime("tomorrow 14:00").expect("parse");
        let dt = Local.timestamp_millis_opt(ms).single().unwrap();
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.date_naive(), Local::now().date_naive() + Duration::days(1));
    }

    #[test]
    fn this_week_is_monday_start() {
        let range = resolve_window(Some("this_week"));
        let start = Local.timestamp_millis_opt(range.start).single().unwrap();
        assert_eq!(start.weekday(), Weekday::Mon);
        assert_eq!(start.hour(), 0);
        assert_eq!(range.end - range.start, 7 * 86_400_000);
    }

    #[test]
    fn next_week_starts_after_this_week() {
        let this = resolve_window(Some("this_week"));
        let next = resolve_window(Some("next_week"));
        assert_eq!(next.start, this.end);
        assert_eq!(next.end - next.start, 7 * 86_400_000);
        let start = Local.timestamp_millis_opt(next.start).single().unwrap();
        assert_eq!(start.weekday(), Weekday::Mon);
        assert_eq!(start.hour(), 0);
    }

    #[test]
    fn parse_when_label_next_week_and_weekend() {
        assert_eq!(parse_when_label("next week plans?"), Some("next_week"));
        assert_eq!(parse_when_label("what's on the weekend"), Some("weekend"));
        assert_eq!(parse_when_label("show me friday"), Some("friday"));
        assert_eq!(parse_when_label("this week's agenda"), Some("this_week"));
        assert_eq!(
            parse_when_label("give me free slots next week not today"),
            Some("next_week")
        );
        assert_eq!(parse_when_label("free slots not today"), Some("not_today"));
        assert_eq!(parse_when_label("give me all week"), Some("this_week"));
    }

    #[test]
    fn not_today_starts_tomorrow() {
        let range = resolve_window(Some("not_today"));
        let start = Local.timestamp_millis_opt(range.start).single().unwrap();
        assert_eq!(start.date_naive(), Local::now().date_naive() + Duration::days(1));
        assert!(range.end > range.start);
    }
}
