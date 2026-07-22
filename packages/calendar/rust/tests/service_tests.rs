use std::sync::Arc;

use buddy_calendar::{
    CreateEventInput, DateRange, EventFilters, RecurrenceRule, Reminder, CalendarService,
};
use buddy_database::Database;
use chrono::TimeZone;
use tempfile::tempdir;

fn open_db() -> Arc<Database> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.db");
    // Keep tempdir alive by leaking — tests are short-lived.
    std::mem::forget(dir);
    Arc::new(Database::open(&path).unwrap())
}

#[tokio::test]
async fn create_list_and_get_event() {
    let db = open_db();
    let svc = CalendarService::with_db(db);
    let now = chrono::Utc::now().timestamp_millis();
    let created = svc
        .create_event(CreateEventInput {
            title: "Standup".into(),
            description: Some("Daily sync".into()),
            location: None,
            category: Some("work".into()),
            color: None,
            start_time: now + 3_600_000,
            end_time: now + 7_200_000,
            all_day: false,
            timezone: Some("UTC".into()),
            recurrence: None,
            reminders: vec![Reminder {
                minutes_before: 15,
                method: "popup".into(),
            }],
            flexibility: None,
            priority: None,
            force: true,
        })
        .await
        .unwrap();

    assert_eq!(created.title, "Standup");
    assert_eq!(created.category, "work");
    assert!(created.color.is_some());

    let listed = svc
        .list_events(
            DateRange {
                start: now,
                end: now + 86_400_000,
            },
            EventFilters::default(),
        )
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);

    let got = svc.get_event(&created.id).await.unwrap();
    assert_eq!(got.id, created.id);
}

#[tokio::test]
async fn duplicate_and_delete() {
    let db = open_db();
    let svc = CalendarService::with_db(db);
    let now = chrono::Utc::now().timestamp_millis();
    let created = svc
        .create_event(CreateEventInput {
            title: "Review".into(),
            description: None,
            location: None,
            category: None,
            color: None,
            start_time: now + 1_000_000,
            end_time: now + 2_000_000,
            all_day: false,
            timezone: None,
            recurrence: None,
            reminders: vec![],
            flexibility: None,
            priority: None,
            force: true,
        })
        .await
        .unwrap();

    let dup = svc.duplicate_event(&created.id).await.unwrap();
    assert!(dup.title.contains("copy"));
    assert_ne!(dup.id, created.id);

    svc.delete_event(&created.id).await.unwrap();
    assert!(svc.get_event(&created.id).await.is_err());
    // Duplicate still exists
    assert!(svc.get_event(&dup.id).await.is_ok());
}

#[tokio::test]
async fn expand_daily_recurrence() {
    let db = open_db();
    let svc = CalendarService::with_db(db);
    let start = chrono::Utc::now().timestamp_millis();
    let _ = svc
        .create_event(CreateEventInput {
            title: "Meditate".into(),
            description: None,
            location: None,
            category: Some("personal".into()),
            color: None,
            start_time: start,
            end_time: start + 1_800_000,
            all_day: false,
            timezone: Some("UTC".into()),
            recurrence: Some(RecurrenceRule {
                frequency: "DAILY".into(),
                interval: 1,
                until: Some(start + 5 * 86_400_000),
                count: None,
                by_day: vec![],
            }),
            reminders: vec![],
            flexibility: None,
            priority: None,
            force: true,
        })
        .await
        .unwrap();

    let listed = svc
        .list_events(
            DateRange {
                start,
                end: start + 4 * 86_400_000,
            },
            EventFilters::default(),
        )
        .await
        .unwrap();
    assert!(listed.len() >= 4);
}

#[tokio::test]
async fn search_by_title() {
    let db = open_db();
    let svc = CalendarService::with_db(db);
    let now = chrono::Utc::now().timestamp_millis();
    svc.create_event(CreateEventInput {
        title: "Budget Planning".into(),
        description: None,
        location: Some("Office".into()),
        category: None,
        color: None,
        start_time: now + 1000,
        end_time: now + 2000,
        all_day: false,
        timezone: None,
        recurrence: None,
        reminders: vec![],
        flexibility: None,
        priority: None,
        force: true,
    })
    .await
    .unwrap();

    let found = svc.search_events("Budget", None).await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title, "Budget Planning");
}

#[tokio::test]
async fn lifestyle_schedule_and_dreams() {
    let db = open_db();
    let svc = CalendarService::with_db(db.clone());
    let now = chrono::Local::now();
    let start = (now - chrono::Duration::days(1)).timestamp_millis();
    let end = (now + chrono::Duration::days(8)).timestamp_millis();
    let blocks = svc.list_schedule_blocks(start, end).await.unwrap();
    assert!(blocks.iter().any(|b| b.kind == buddy_calendar::ScheduleKind::Work));
    assert!(blocks.iter().any(|b| b.kind == buddy_calendar::ScheduleKind::Sleep));

    let overnight = blocks
        .iter()
        .find(|b| b.kind == buddy_calendar::ScheduleKind::Sleep)
        .unwrap();
    assert!(overnight.end_time > overnight.start_time);
    // Sleep crosses midnight → duration > 6h
    assert!(overnight.end_time - overnight.start_time > 6 * 3_600_000);

    let dream = svc
        .log_dream(buddy_calendar::CreateDreamInput {
            body: "I flew over mountains".into(),
            sleep_date: Some(overnight.anchor_date.clone()),
            title: Some("Flying".into()),
            tags: vec!["vivid".into()],
            mood: None,
            sleep_quality: None,
        })
        .await
        .unwrap();
    assert_eq!(dream.sleep_date, overnight.anchor_date);

    let listed = svc.list_dreams(&overnight.anchor_date).await.unwrap();
    assert_eq!(listed.len(), 1);

    let found = svc.search_dreams("mountains").await.unwrap();
    assert_eq!(found.len(), 1);
}

#[tokio::test]
async fn work_sales_and_hours() {
    let db = open_db();
    let svc = CalendarService::with_db(db);
    let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
    // Only assert sales path; hours depend on weekday.
    let log = svc
        .log_work_sales(Some(today.clone()), 320.0, Some("GBP".into()))
        .await
        .unwrap();
    assert!((log.sales_amount - 320.0).abs() < f64::EPSILON);

    let end = chrono::Local::now()
        .date_naive()
        .and_hms_opt(17, 15, 0)
        .unwrap();
    let end_ms = chrono::Local
        .from_local_datetime(&end)
        .single()
        .unwrap()
        .timestamp_millis();
    let _ = svc
        .set_work_hours(Some(today), None, Some(end_ms))
        .await
        .unwrap();
    let stats = svc.get_work_stats().await.unwrap();
    assert!((stats.today.sales - 320.0).abs() < f64::EPSILON);
}
