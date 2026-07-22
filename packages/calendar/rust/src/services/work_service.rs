use chrono::{Datelike, Duration, Local, NaiveDate};
use buddy_database::{chrono_now, Database, WorkDayLogRow};

use crate::error::CalendarError;
use crate::models::{WorkDayLog, WorkPeriodStats, WorkStats};
use crate::services::schedule_service::{
    hours_between, parse_date, template_work_bounds, today_date_string,
};

fn row_to_log(row: WorkDayLogRow) -> WorkDayLog {
    WorkDayLog {
        work_date: row.work_date,
        actual_start_ms: row.actual_start_ms,
        actual_end_ms: row.actual_end_ms,
        sales_amount: row.sales_amount,
        sales_currency: row.sales_currency,
        notes: row.notes,
        updated_at: row.updated_at,
    }
}

fn empty_log(work_date: &str) -> WorkDayLogRow {
    WorkDayLogRow {
        work_date: work_date.to_string(),
        actual_start_ms: None,
        actual_end_ms: None,
        sales_amount: 0.0,
        sales_currency: "GBP".into(),
        notes: None,
        updated_at: chrono_now(),
    }
}

pub fn get_or_empty(db: &Database, work_date: &str) -> Result<WorkDayLog, CalendarError> {
    Ok(match db.get_work_day_log(work_date)? {
        Some(row) => row_to_log(row),
        None => row_to_log(empty_log(work_date)),
    })
}

pub fn hours_for_date(db: &Database, work_date: &str) -> Result<f64, CalendarError> {
    let log = db.get_work_day_log(work_date)?;
    if let Some(row) = &log {
        if let (Some(s), Some(e)) = (row.actual_start_ms, row.actual_end_ms) {
            return Ok(hours_between(s, e));
        }
        if let Some(e) = row.actual_end_ms {
            if let Some((ts, _)) = template_work_bounds(db, work_date)? {
                return Ok(hours_between(ts, e));
            }
        }
        if let Some(s) = row.actual_start_ms {
            if let Some((_, te)) = template_work_bounds(db, work_date)? {
                return Ok(hours_between(s, te));
            }
        }
    }
    match template_work_bounds(db, work_date)? {
        Some((s, e)) => Ok(hours_between(s, e)),
        None => Ok(0.0),
    }
}

fn sum_hours(db: &Database, dates: &[String]) -> Result<f64, CalendarError> {
    let mut total = 0.0;
    for d in dates {
        total += hours_for_date(db, d)?;
    }
    Ok(total)
}

fn sum_sales(db: &Database, start: &str, end: &str) -> Result<(f64, String), CalendarError> {
    let logs = db.list_work_day_logs(start, end)?;
    let sales: f64 = logs.iter().map(|l| l.sales_amount).sum();
    let currency = logs
        .first()
        .map(|l| l.sales_currency.clone())
        .unwrap_or_else(|| "GBP".into());
    Ok((sales, currency))
}

fn dates_inclusive(start: NaiveDate, end: NaiveDate) -> Vec<String> {
    let mut out = Vec::new();
    let mut d = start;
    while d <= end {
        out.push(d.format("%Y-%m-%d").to_string());
        d += Duration::days(1);
    }
    out
}

pub fn get_stats(db: &Database) -> Result<WorkStats, CalendarError> {
    let today = Local::now().date_naive();
    let today_s = today.format("%Y-%m-%d").to_string();

    let weekday = today.weekday().num_days_from_monday() as i64;
    let week_start = today - Duration::days(weekday);
    let week_end = week_start + Duration::days(6);
    let month_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
    let _month_end = if today.month() == 12 {
        NaiveDate::from_ymd_opt(today.year() + 1, 1, 1).unwrap() - Duration::days(1)
    } else {
        NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1).unwrap() - Duration::days(1)
    };

    let week_dates = dates_inclusive(week_start, week_end.min(today));
    let month_dates = dates_inclusive(month_start, today);

    let today_hours = hours_for_date(db, &today_s)?;
    let week_hours = sum_hours(db, &week_dates)?;
    let month_hours = sum_hours(db, &month_dates)?;

    let (today_sales, currency) = sum_sales(db, &today_s, &today_s)?;
    let (week_sales, _) = sum_sales(
        db,
        &week_start.format("%Y-%m-%d").to_string(),
        &today_s,
    )?;
    let (month_sales, _) = sum_sales(
        db,
        &month_start.format("%Y-%m-%d").to_string(),
        &today_s,
    )?;

    Ok(WorkStats {
        today: WorkPeriodStats {
            hours: today_hours,
            sales: today_sales,
            currency: currency.clone(),
        },
        week: WorkPeriodStats {
            hours: week_hours,
            sales: week_sales,
            currency: currency.clone(),
        },
        month: WorkPeriodStats {
            hours: month_hours,
            sales: month_sales,
            currency,
        },
    })
}

pub fn log_sales(
    db: &Database,
    work_date: Option<String>,
    amount: f64,
    currency: Option<String>,
) -> Result<WorkDayLog, CalendarError> {
    let date = work_date
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(today_date_string);
    let _ = parse_date(&date)?;
    let mut row = db
        .get_work_day_log(&date)?
        .unwrap_or_else(|| empty_log(&date));
    row.sales_amount = amount;
    if let Some(c) = currency.filter(|c| !c.trim().is_empty()) {
        row.sales_currency = c;
    }
    row.updated_at = chrono_now();
    db.upsert_work_day_log(&row)?;
    Ok(row_to_log(row))
}

pub fn set_hours(
    db: &Database,
    work_date: Option<String>,
    actual_start_ms: Option<i64>,
    actual_end_ms: Option<i64>,
) -> Result<WorkDayLog, CalendarError> {
    let date = work_date
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(today_date_string);
    let _ = parse_date(&date)?;
    let mut row = db
        .get_work_day_log(&date)?
        .unwrap_or_else(|| empty_log(&date));
    if actual_start_ms.is_some() {
        row.actual_start_ms = actual_start_ms;
    }
    if actual_end_ms.is_some() {
        row.actual_end_ms = actual_end_ms;
    }
    if let (Some(s), Some(e)) = (row.actual_start_ms, row.actual_end_ms) {
        if e <= s {
            return Err(CalendarError::InvalidInput(
                "work end must be after start".into(),
            ));
        }
    }
    row.updated_at = chrono_now();
    db.upsert_work_day_log(&row)?;
    Ok(row_to_log(row))
}
