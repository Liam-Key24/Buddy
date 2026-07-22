use std::sync::Arc;

use buddy_calendar::{
    scheduling::{
        block_focus_time, build_occupancy, compute_day_capacity, compose_day_summary,
        detect_conflicts, find_free_slots, plan_day, reschedule_flexible, schedule_items,
        BusySource, PlanDayRequest, ScheduleItem, SchedulingContext, SchedulingPolicy,
    },
    CreateEventInput, DateRange, Event, EventPriority, Flexibility, ScheduleBlock, ScheduleKind,
    CalendarService, WriteEventOutcome,
};
use buddy_database::Database;
use chrono::{Duration, TimeZone, Utc};
use tempfile::tempdir;

fn open_db() -> Arc<Database> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("sched.db");
    std::mem::forget(dir);
    Arc::new(Database::open(&path).unwrap())
}

fn monday_noon() -> i64 {
    // Fixed Monday 2026-07-20 12:00 UTC
    Utc.with_ymd_and_hms(2026, 7, 20, 12, 0, 0)
        .unwrap()
        .timestamp_millis()
}

fn day_range(day_ms: i64) -> DateRange {
    let dt = Utc.timestamp_millis_opt(day_ms).single().unwrap();
    let start = Utc
        .from_utc_datetime(&dt.date_naive().and_hms_opt(0, 0, 0).unwrap())
        .timestamp_millis();
    DateRange {
        start,
        end: start + Duration::days(1).num_milliseconds(),
    }
}

fn make_event(title: &str, start: i64, end: i64, flex: Flexibility) -> Event {
    Event {
        id: format!("e-{title}"),
        title: title.into(),
        description: None,
        location: None,
        category: "personal".into(),
        color: None,
        start_time: start,
        end_time: end,
        all_day: false,
        timezone: "UTC".into(),
        recurrence: None,
        reminders: vec![],
        external_provider: None,
        external_event_id: None,
        sync_status: "local".into(),
        created_at: 0,
        updated_at: 0,
        occurrence_of: None,
        flexibility: flex,
        priority: EventPriority::Normal,
    }
}

fn sleep_work_blocks(day_ms: i64) -> Vec<ScheduleBlock> {
    let range = day_range(day_ms);
    let date = Utc
        .timestamp_millis_opt(range.start)
        .single()
        .unwrap()
        .format("%Y-%m-%d")
        .to_string();
    // Sleep previous night into morning: 22:30 prev → 07:45
    let sleep_start = range.start
        - Duration::hours(1).num_milliseconds()
        - Duration::minutes(30).num_milliseconds();
    let sleep_end = range.start
        + Duration::hours(7).num_milliseconds()
        + Duration::minutes(45).num_milliseconds();
    // Evening sleep start same day 22:30 → next morning (clipped by range)
    let evening_sleep_start = range.start
        + Duration::hours(22).num_milliseconds()
        + Duration::minutes(30).num_milliseconds();
    // Work 08:45–16:45
    let work_start = range.start
        + Duration::hours(8).num_milliseconds()
        + Duration::minutes(45).num_milliseconds();
    let work_end = range.start
        + Duration::hours(16).num_milliseconds()
        + Duration::minutes(45).num_milliseconds();
    vec![
        ScheduleBlock {
            id: format!("sleep::{date}"),
            kind: ScheduleKind::Sleep,
            title: "Sleep".into(),
            start_time: sleep_start,
            end_time: sleep_end,
            anchor_date: date.clone(),
        },
        ScheduleBlock {
            id: format!("sleep::{date}-eve"),
            kind: ScheduleKind::Sleep,
            title: "Sleep".into(),
            start_time: evening_sleep_start,
            end_time: range.end + Duration::hours(7).num_milliseconds(),
            anchor_date: date.clone(),
        },
        ScheduleBlock {
            id: format!("work::{date}"),
            kind: ScheduleKind::Work,
            title: "Work".into(),
            start_time: work_start,
            end_time: work_end,
            anchor_date: date,
        },
    ]
}

fn ctx_with(events: Vec<Event>, day_ms: i64) -> SchedulingContext {
    let range = day_range(day_ms);
    SchedulingContext::new(
        events,
        sleep_work_blocks(day_ms),
        SchedulingPolicy::default(),
        range,
    )
}

#[test]
fn protected_sleep_cannot_be_scheduled_over() {
    let day = monday_noon();
    let ctx = ctx_with(vec![], day);
    let sleep = ctx
        .lifestyle_blocks
        .iter()
        .find(|b| b.kind == ScheduleKind::Sleep)
        .unwrap();
    // Probe inside the clipped overnight sleep on this calendar day (e.g. 01:00–02:00).
    let probe_start = day_range(day).start + Duration::hours(1).num_milliseconds();
    let probe_end = probe_start + Duration::hours(1).num_milliseconds();
    assert!(probe_start >= sleep.start_time.max(day_range(day).start));
    let report = detect_conflicts(&ctx, probe_start, probe_end, None);
    assert!(report.has_conflicts);
    assert!(report
        .conflicts
        .iter()
        .any(|c| matches!(c.kind, buddy_calendar::scheduling::ConflictKind::ProtectedSleep)));
}

#[test]
fn protected_work_cannot_be_scheduled_over() {
    let day = monday_noon();
    let ctx = ctx_with(vec![], day);
    let work = ctx
        .lifestyle_blocks
        .iter()
        .find(|b| b.kind == ScheduleKind::Work)
        .unwrap();
    let report = detect_conflicts(&ctx, work.start_time + 60_000, work.start_time + 3_600_000, None);
    assert!(report.has_conflicts);
    assert!(report
        .conflicts
        .iter()
        .any(|c| matches!(c.kind, buddy_calendar::scheduling::ConflictKind::ProtectedWork)));
}

#[test]
fn buffer_times_inserted_in_occupancy() {
    let day = monday_noon();
    let range = day_range(day);
    // Evening free window after work: 18:00–19:00 event
    let start = range.start + Duration::hours(18).num_milliseconds();
    let end = start + Duration::hours(1).num_milliseconds();
    let ctx = ctx_with(
        vec![make_event("Call", start, end, Flexibility::Fixed)],
        day,
    );
    let occ = build_occupancy(&ctx);
    assert!(occ.iter().any(|b| b.source == BusySource::Buffer));
    let buffers: Vec<_> = occ
        .iter()
        .filter(|b| b.source == BusySource::Buffer)
        .collect();
    assert!(buffers.iter().any(|b| b.end == start));
    assert!(buffers.iter().any(|b| b.start == end));
    assert_eq!(buffers[0].end - buffers[0].start, 10 * 60_000);
}

#[test]
fn conflicts_detect_overlap_and_buffer() {
    let day = monday_noon();
    let range = day_range(day);
    let start = range.start + Duration::hours(18).num_milliseconds();
    let end = start + Duration::hours(1).num_milliseconds();
    let ctx = ctx_with(
        vec![make_event("Existing", start, end, Flexibility::Fixed)],
        day,
    );
    let overlap = detect_conflicts(&ctx, start + 30 * 60_000, end + 30 * 60_000, None);
    assert!(overlap.has_conflicts);
    assert!(overlap
        .conflicts
        .iter()
        .any(|c| matches!(c.kind, buddy_calendar::scheduling::ConflictKind::Overlap)));

    // Immediately after existing (within buffer)
    let buf = detect_conflicts(&ctx, end, end + 30 * 60_000, None);
    assert!(buf.has_conflicts);
    assert!(buf.conflicts.iter().any(|c| matches!(
        c.kind,
        buddy_calendar::scheduling::ConflictKind::BufferViolation
    )));
}

#[test]
fn flexible_can_reschedule_fixed_cannot() {
    let day = monday_noon();
    let range = day_range(day);
    let start = range.start + Duration::hours(19).num_milliseconds();
    let end = start + Duration::hours(1).num_milliseconds();
    let fixed = make_event("Fixed", start, end, Flexibility::Fixed);
    let flex = make_event("Flex", start, end, Flexibility::Flexible);
    let ctx = ctx_with(vec![fixed.clone(), flex.clone()], day);

    assert!(reschedule_flexible(&ctx, &fixed).is_err());
    let slots = reschedule_flexible(&ctx, &flex).unwrap();
    assert!(!slots.is_empty());
}

#[test]
fn free_time_respects_protected_blocks() {
    let day = monday_noon();
    let ctx = ctx_with(vec![], day);
    let slots = find_free_slots(&ctx, 60 * 60_000, 20, None);
    assert!(!slots.is_empty());
    for slot in &slots {
        let report = detect_conflicts(&ctx, slot.start, slot.end, None);
        assert!(
            !report.has_conflicts,
            "slot {:?} conflicted: {:?}",
            slot,
            report.conflicts
        );
    }
}

#[test]
fn plan_day_creates_non_overlapping_schedule() {
    let day = monday_noon();
    let ctx = ctx_with(vec![], day);
    let result = plan_day(
        &ctx,
        &PlanDayRequest {
            day,
            tasks: vec![
                ScheduleItem {
                    title: "Deep work".into(),
                    duration_minutes: 60,
                    deadline: None,
                    priority: Some(EventPriority::High),
                    flexibility: Some(Flexibility::Flexible),
                    category: Some("personal".into()),
                    description: None,
                },
                ScheduleItem {
                    title: "Email".into(),
                    duration_minutes: 30,
                    deadline: None,
                    priority: Some(EventPriority::Normal),
                    flexibility: Some(Flexibility::Flexible),
                    category: None,
                    description: None,
                },
            ],
            include_breaks: true,
            apply: false,
        },
    );
    assert!(result.proposed.len() >= 1);
    for (i, a) in result.proposed.iter().enumerate() {
        for (j, b) in result.proposed.iter().enumerate() {
            if i >= j {
                continue;
            }
            assert!(
                a.end + 10 * 60_000 <= b.start || b.end + 10 * 60_000 <= a.start,
                "proposed blocks overlap/buffer violate"
            );
        }
        let report = detect_conflicts(&ctx, a.start, a.end, None);
        assert!(!report.has_conflicts);
    }
}

#[test]
fn auto_schedule_never_overlaps() {
    let day = monday_noon();
    let mut ctx = ctx_with(vec![], day);
    // Expand search to a week for more room
    ctx.range.end = ctx.range.start + Duration::days(5).num_milliseconds();
    let result = schedule_items(
        &ctx,
        &[
            ScheduleItem {
                title: "A".into(),
                duration_minutes: 90,
                deadline: None,
                priority: None,
                flexibility: None,
                category: None,
                description: None,
            },
            ScheduleItem {
                title: "B".into(),
                duration_minutes: 90,
                deadline: None,
                priority: None,
                flexibility: None,
                category: None,
                description: None,
            },
        ],
    );
    assert_eq!(result.scheduled.len(), 2);
    let a = &result.scheduled[0];
    let b = &result.scheduled[1];
    assert!(a.end + 10 * 60_000 <= b.start || b.end + 10 * 60_000 <= a.start);
}

#[test]
fn capacity_calculations_are_accurate() {
    let day = monday_noon();
    let range = day_range(day);
    let start = range.start + Duration::hours(18).num_milliseconds();
    let end = start + Duration::hours(2).num_milliseconds();
    let ctx = ctx_with(
        vec![make_event("Focus block", start, end, Flexibility::Flexible)],
        day,
    );
    let cap = compute_day_capacity(&ctx, day);
    assert!((cap.focus_hours - 2.0).abs() < 0.01);
    assert!(cap.booked_hours >= 2.0);
    assert!(cap.waking_hours > 0.0);
}

#[test]
fn fragmented_day_suggests_focus() {
    let day = monday_noon();
    let range = day_range(day);
    // Fill post-work free time with back-to-back meetings until evening sleep.
    let mut events = Vec::new();
    let mut t = range.start + Duration::hours(17).num_milliseconds();
    let evening_sleep = range.start
        + Duration::hours(22).num_milliseconds()
        + Duration::minutes(30).num_milliseconds();
    let mut i = 0;
    while t + 45 * 60_000 <= evening_sleep {
        events.push(make_event(
            &format!("Meeting {i}"),
            t,
            t + 40 * 60_000,
            Flexibility::Fixed,
        ));
        t += 45 * 60_000;
        i += 1;
    }
    for e in &mut events {
        e.category = "work".into();
    }
    assert!(events.len() >= 3);
    let ctx = ctx_with(events, day);
    let summary = compose_day_summary(&ctx, day);
    assert!(
        summary.suggestions.iter().any(|s| matches!(
            s.action,
            buddy_calendar::scheduling::SuggestionAction::ProtectFocus
        ) || s.message.to_ascii_lowercase().contains("focus")),
        "expected focus suggestion, got {:?}",
        summary.suggestions
    );
}

#[test]
fn block_focus_time_avoids_protected() {
    let day = monday_noon();
    let ctx = ctx_with(vec![], day);
    let block = block_focus_time(&ctx, "Coding", 120).expect("should find focus slot");
    let report = detect_conflicts(&ctx, block.start, block.end, None);
    assert!(!report.has_conflicts);
    assert_eq!(block.end - block.start, 120 * 60_000);
}

#[test]
fn cooking_dinner_prefers_evening_not_morning() {
    let day = monday_noon();
    let ctx = ctx_with(vec![], day);
    let result = schedule_items(
        &ctx,
        &[
            ScheduleItem {
                title: "tennis".into(),
                duration_minutes: 60,
                deadline: None,
                priority: None,
                flexibility: Some(Flexibility::Flexible),
                category: None,
                description: None,
            },
            ScheduleItem {
                title: "bath".into(),
                duration_minutes: 60,
                deadline: None,
                priority: None,
                flexibility: Some(Flexibility::Flexible),
                category: None,
                description: None,
            },
            ScheduleItem {
                title: "cooking dinner".into(),
                duration_minutes: 60,
                deadline: None,
                priority: None,
                flexibility: Some(Flexibility::Flexible),
                category: None,
                description: None,
            },
        ],
    );
    assert_eq!(result.scheduled.len(), 3);
    let dinner = result
        .scheduled
        .iter()
        .find(|b| b.title.contains("cooking"))
        .expect("dinner scheduled");
    let hour = {
        use chrono::{Local, TimeZone, Timelike};
        Local
            .timestamp_millis_opt(dinner.start)
            .single()
            .map(|d| d.hour())
            .unwrap_or(0)
    };
    assert!(
        (16..=21).contains(&hour),
        "cooking dinner should be evening, got hour {hour} ({dinner:?})"
    );
}

#[tokio::test]
async fn create_event_checked_returns_conflict_on_sleep() {
    let db = open_db();
    let svc = CalendarService::with_db(db);
    let day = monday_noon();
    let blocks = svc
        .list_schedule_blocks(day - 86_400_000, day + 86_400_000)
        .await
        .unwrap();
    let sleep = blocks
        .iter()
        .find(|b| b.kind == ScheduleKind::Sleep)
        .expect("seeded sleep");
    let outcome = svc
        .create_event_checked(CreateEventInput {
            title: "Bad".into(),
            description: None,
            location: None,
            category: None,
            color: None,
            start_time: sleep.start_time + 60_000,
            end_time: sleep.start_time + 3_600_000,
            all_day: false,
            timezone: None,
            recurrence: None,
            reminders: vec![],
            flexibility: None,
            priority: None,
            force: false,
        })
        .await
        .unwrap();
    match outcome {
        WriteEventOutcome::Conflict { report } => assert!(report.has_conflicts),
        WriteEventOutcome::Ok { .. } => panic!("expected conflict"),
    }
}

#[tokio::test]
async fn force_create_writes_despite_conflict() {
    let db = open_db();
    let svc = CalendarService::with_db(db);
    let day = monday_noon();
    let blocks = svc
        .list_schedule_blocks(day - 86_400_000, day + 86_400_000)
        .await
        .unwrap();
    let sleep = blocks
        .iter()
        .find(|b| b.kind == ScheduleKind::Sleep)
        .unwrap();
    let created = svc
        .create_event(CreateEventInput {
            title: "Forced".into(),
            description: None,
            location: None,
            category: None,
            color: None,
            start_time: sleep.start_time + 60_000,
            end_time: sleep.start_time + 3_600_000,
            all_day: false,
            timezone: None,
            recurrence: None,
            reminders: vec![],
            flexibility: Some(Flexibility::Fixed),
            priority: None,
            force: true,
        })
        .await
        .unwrap();
    assert_eq!(created.title, "Forced");
}
