use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::models::{Event, EventPriority, Flexibility};
use crate::scheduling::capacity::compute_day_capacity;
use crate::scheduling::conflict::detect_conflicts;
use crate::scheduling::free_time::{find_free_slots, find_free_slots_for};
use crate::scheduling::occupancy::{build_occupancy, BusySource};
use crate::scheduling::types::{DayCapacity, FreeSlot, Suggestion, SuggestionAction};
use crate::scheduling::{default_task_flexibility, default_task_priority, SchedulingContext};

fn parse_millis_value(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => {
            let trimmed = s.trim();
            if let Ok(n) = trimmed.parse::<i64>() {
                return Some(n);
            }
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(trimmed) {
                return Some(dt.timestamp_millis());
            }
            if let Ok(naive) =
                chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S")
            {
                return Some(naive.and_utc().timestamp_millis());
            }
            if let Ok(naive) =
                chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S")
            {
                return Some(naive.and_utc().timestamp_millis());
            }
            if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M")
            {
                return Some(naive.and_utc().timestamp_millis());
            }
            None
        }
        _ => None,
    }
}

fn deserialize_opt_millis<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(v) => parse_millis_value(&v)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid datetime: {v}"))),
    }
}

/// Task/work item accepted by scheduling APIs (Calendar tools + future Tasks plugin).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScheduleItem {
    pub title: String,
    pub duration_minutes: u32,
    #[serde(default, deserialize_with = "deserialize_opt_millis")]
    pub deadline: Option<i64>,
    #[serde(default)]
    pub priority: Option<EventPriority>,
    #[serde(default)]
    pub flexibility: Option<Flexibility>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Place this item `count` times (capped). Expanded before placement.
    #[serde(default)]
    pub count: Option<u32>,
    /// Prefer different local days when placing repeated occurrences.
    #[serde(default)]
    pub prefer_spread: Option<bool>,
    /// Prefer slots after lifestyle Work end for that day (calendar Work hours).
    #[serde(default)]
    pub prefer_after_work: Option<bool>,
    /// Optional spark id this block is working on (recorded in description).
    #[serde(default)]
    pub spark_id: Option<String>,
}

/// Expand `count` into individual items (max 14 per item).
pub fn expand_schedule_items(items: &[ScheduleItem]) -> Vec<ScheduleItem> {
    let mut out = Vec::new();
    for item in items {
        let n = item.count.unwrap_or(1).clamp(1, 14);
        for _ in 0..n {
            let mut copy = item.clone();
            copy.count = None;
            if let Some(sid) = &item.spark_id {
                let tag = format!("spark:{sid}");
                copy.description = Some(match copy.description {
                    Some(d) if !d.is_empty() => format!("{d}\n{tag}"),
                    _ => tag,
                });
            }
            out.push(copy);
        }
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedBlock {
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub flexibility: Flexibility,
    pub priority: EventPriority,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub score: f64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanDayRequest {
    /// Unix ms somewhere within the target day.
    pub day: i64,
    #[serde(default)]
    pub tasks: Vec<ScheduleItem>,
    #[serde(default = "default_true")]
    pub include_breaks: bool,
    /// When true, caller will persist proposed blocks.
    #[serde(default)]
    pub apply: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanDayResult {
    pub date: String,
    pub proposed: Vec<ProposedBlock>,
    pub capacity: DayCapacity,
    pub suggestions: Vec<Suggestion>,
    pub unscheduled: Vec<ScheduleItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleTradeoff {
    pub new_title: String,
    pub blocking_event_id: String,
    pub blocking_title: String,
    pub blocking_start: i64,
    pub blocking_end: i64,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleItemsResult {
    pub scheduled: Vec<ProposedBlock>,
    pub unscheduled: Vec<ScheduleItem>,
    pub suggestions: Vec<Suggestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tradeoff: Option<ScheduleTradeoff>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OrganizeMode {
    #[default]
    Propose,
    Commit,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrganizeItemIn {
    pub title: String,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub duration_minutes: Option<u32>,
    #[serde(default)]
    pub when: Option<String>,
    #[serde(default)]
    pub count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizeResult {
    pub status: String,
    pub window_start: i64,
    pub window_end: i64,
    pub scheduled: Vec<ProposedBlock>,
    pub unscheduled: Vec<ScheduleItem>,
    pub suggestions: Vec<Suggestion>,
    pub lifted_ids: Vec<String>,
    pub apply: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tradeoff: Option<ScheduleTradeoff>,
}

/// Place schedule items into the best scored free slots without overlapping.
pub fn schedule_items(
    ctx: &SchedulingContext,
    items: &[ScheduleItem],
) -> ScheduleItemsResult {
    let mut working = ctx.clone();
    let mut scheduled = Vec::new();
    let mut unscheduled = Vec::new();
    let mut suggestions = Vec::new();
    let mut tradeoff = None;

    let mut ordered: Vec<ScheduleItem> = expand_schedule_items(items);
    // Meal/dinner tasks first so they claim evening before generic evening fillers.
    ordered.sort_by(|a, b| {
        let pa = a.priority.unwrap_or(default_task_priority()).rank();
        let pb = b.priority.unwrap_or(default_task_priority()).rank();
        let ma = meal_sort_key(&a.title);
        let mb = meal_sort_key(&b.title);
        pb.cmp(&pa)
            .then_with(|| ma.cmp(&mb))
            .then_with(|| match (a.deadline, b.deadline) {
                (Some(da), Some(db)) => da.cmp(&db),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            })
    });

    for item in ordered {
        let duration_ms = (item.duration_minutes as i64).max(1) * 60_000;
        let mut search_ctx = working.clone();
        if let Some(deadline) = item.deadline {
            search_ctx.range.end = search_ctx.range.end.min(deadline);
        }
        let prefer_spread = item.prefer_spread.unwrap_or(false);
        let prefer_after_work = item.prefer_after_work.unwrap_or(false);
        let used_days = used_local_days_for_title(&scheduled, &item.title);
        let slots = find_free_slots_for(
            &search_ctx,
            duration_ms,
            if prefer_spread { 24 } else { 12 },
            None,
            Some(item.title.as_str()),
            prefer_after_work,
        );
        let best = pick_slot(&slots, prefer_spread, &used_days);
        let Some(best) = best.cloned() else {
            let reason = format!(
                "Could not find a free slot for \"{}\" ({} min) without violating protections.",
                item.title, item.duration_minutes
            );
            record_unscheduled(
                item,
                &working,
                duration_ms,
                prefer_after_work,
                reason,
                &mut suggestions,
                &mut unscheduled,
                &mut tradeoff,
            );
            continue;
        };

        // Capacity guard: skip if day becomes overloaded.
        let day_cap = compute_day_capacity(&working, best.start);
        let added_hours = duration_ms as f64 / 3_600_000.0;
        if day_cap.overloaded
            || (day_cap.booked_hours + added_hours) / day_cap.waking_hours.max(0.1)
                >= working.policy.overload_threshold
        {
            // Try next slots on other days if available.
            let alt = slots.iter().find(|s| {
                let day_key = local_day_key(s.start);
                if prefer_spread && used_days.contains(&day_key) && !used_days.is_empty() {
                    return false;
                }
                let cap = compute_day_capacity(&working, s.start);
                !cap.overloaded
                    && (cap.booked_hours + added_hours) / cap.waking_hours.max(0.1)
                        < working.policy.overload_threshold
            }).or_else(|| {
                slots.iter().find(|s| {
                    let cap = compute_day_capacity(&working, s.start);
                    !cap.overloaded
                        && (cap.booked_hours + added_hours) / cap.waking_hours.max(0.1)
                            < working.policy.overload_threshold
                })
            });
            let Some(best) = alt.cloned() else {
                let reason = format!(
                    "Skipping \"{}\" to avoid overloading the day.",
                    item.title
                );
                record_unscheduled(
                    item,
                    &working,
                    duration_ms,
                    prefer_after_work,
                    reason,
                    &mut suggestions,
                    &mut unscheduled,
                    &mut tradeoff,
                );
                continue;
            };
            push_proposed(&mut scheduled, &mut working, &item, &best);
            continue;
        }

        push_proposed(&mut scheduled, &mut working, &item, &best);
    }

    scheduled.sort_by_key(|b| b.start);

    ScheduleItemsResult {
        scheduled,
        unscheduled,
        suggestions,
        tradeoff,
    }
}

fn item_needs_tradeoff(item: &ScheduleItem) -> bool {
    item.deadline.is_some()
        || item
            .priority
            .unwrap_or_else(default_task_priority)
            .rank()
            >= EventPriority::High.rank()
}

fn record_unscheduled(
    item: ScheduleItem,
    working: &SchedulingContext,
    duration_ms: i64,
    prefer_after_work: bool,
    message: String,
    suggestions: &mut Vec<Suggestion>,
    unscheduled: &mut Vec<ScheduleItem>,
    tradeoff: &mut Option<ScheduleTradeoff>,
) {
    if tradeoff.is_none() && item_needs_tradeoff(&item) {
        if let Some(found) = find_tradeoff(working, &item, duration_ms, prefer_after_work) {
            suggestions.push(Suggestion {
                action: SuggestionAction::MoveNew,
                message: found.prompt.clone(),
                event_id: Some(found.blocking_event_id.clone()),
                start: Some(found.blocking_start),
                end: Some(found.blocking_end),
            });
            *tradeoff = Some(found);
            unscheduled.push(item);
            return;
        }
    }
    suggestions.push(Suggestion {
        action: SuggestionAction::Redistribute,
        message,
        event_id: None,
        start: None,
        end: None,
    });
    unscheduled.push(item);
}

/// If lifting a lower-priority movable event frees a slot, offer a choice.
fn find_tradeoff(
    ctx: &SchedulingContext,
    item: &ScheduleItem,
    duration_ms: i64,
    prefer_after_work: bool,
) -> Option<ScheduleTradeoff> {
    let new_rank = item
        .priority
        .unwrap_or_else(default_task_priority)
        .rank();
    let mut candidates: Vec<&Event> = ctx
        .events
        .iter()
        .filter(|e| e.flexibility.is_movable() && !e.all_day)
        .filter(|e| e.priority.rank() <= new_rank)
        .filter(|e| e.end_time > ctx.range.start && e.start_time < ctx.range.end)
        .collect();
    candidates.sort_by_key(|e| std::cmp::Reverse(e.start_time));

    for ev in candidates {
        let slots = find_free_slots_for(
            ctx,
            duration_ms,
            8,
            Some(ev.id.as_str()),
            Some(item.title.as_str()),
            prefer_after_work,
        );
        if slots.is_empty() {
            continue;
        }
        return Some(ScheduleTradeoff {
            new_title: item.title.clone(),
            blocking_event_id: ev.id.clone(),
            blocking_title: ev.title.clone(),
            blocking_start: ev.start_time,
            blocking_end: ev.end_time,
            prompt: format!(
                "You have \"{}\" that day. Move it so \"{}\" can land, or do {} later?",
                ev.title, item.title, item.title
            ),
        });
    }
    None
}

fn local_day_key(ms: i64) -> i64 {
    crate::scheduling::local_day_bounds_ms(ms).0
}

fn used_local_days_for_title(scheduled: &[ProposedBlock], title: &str) -> std::collections::HashSet<i64> {
    scheduled
        .iter()
        .filter(|b| b.title.eq_ignore_ascii_case(title))
        .map(|b| local_day_key(b.start))
        .collect()
}

fn pick_slot<'a>(
    slots: &'a [FreeSlot],
    prefer_spread: bool,
    used_days: &std::collections::HashSet<i64>,
) -> Option<&'a FreeSlot> {
    if slots.is_empty() {
        return None;
    }
    if prefer_spread && !used_days.is_empty() {
        if let Some(spread) = slots
            .iter()
            .find(|s| !used_days.contains(&local_day_key(s.start)))
        {
            return Some(spread);
        }
    }
    slots.first()
}

/// Lower sorts first. Meals before sports/chores so dinner keeps the evening.
fn meal_sort_key(title: &str) -> u8 {
    let t = title.to_ascii_lowercase();
    if t.contains("dinner")
        || t.contains("supper")
        || t.contains("lunch")
        || t.contains("breakfast")
        || t.contains("cook")
    {
        0
    } else if t.contains("bath") || t.contains("shower") {
        2
    } else {
        1
    }
}

fn push_proposed(
    scheduled: &mut Vec<ProposedBlock>,
    working: &mut SchedulingContext,
    item: &ScheduleItem,
    slot: &FreeSlot,
) {
    let flexibility = item.flexibility.unwrap_or_else(default_task_flexibility);
    let priority = item.priority.unwrap_or_else(default_task_priority);
    scheduled.push(ProposedBlock {
        title: item.title.clone(),
        start: slot.start,
        end: slot.end,
        flexibility,
        priority,
        category: item.category.clone(),
        description: item.description.clone(),
        score: slot.score,
        reasons: slot.reasons.clone(),
    });
    // Occupy the slot so subsequent items don't collide (including buffers).
    working.events.push(Event {
        id: format!("proposed::{}", scheduled.len()),
        title: item.title.clone(),
        description: None,
        location: None,
        category: item.category.clone().unwrap_or_else(|| "general".into()),
        color: None,
        start_time: slot.start,
        end_time: slot.end,
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
        flexibility,
        priority,
    });
}

/// Smart time blocking: best uninterrupted period for a focus block.
pub fn block_focus_time(
    ctx: &SchedulingContext,
    title: &str,
    duration_minutes: u32,
) -> Option<ProposedBlock> {
    let duration_ms = (duration_minutes as i64).max(1) * 60_000;
    let slots = find_free_slots_for(ctx, duration_ms, 5, None, Some(title), false);
    let best = slots.into_iter().next()?;
    Some(ProposedBlock {
        title: title.to_string(),
        start: best.start,
        end: best.end,
        flexibility: Flexibility::Flexible,
        priority: EventPriority::Normal,
        category: Some("personal".into()),
        description: Some("Focus block".into()),
        score: best.score,
        reasons: best.reasons,
    })
}

/// Plan a day: schedule tasks, optionally insert short breaks, return proposals.
pub fn plan_day(ctx: &SchedulingContext, request: &PlanDayRequest) -> PlanDayResult {
    let (day_start, day_end) = crate::scheduling::local_day_bounds_ms(request.day);

    let mut day_ctx = ctx.clone();
    day_ctx.range.start = day_start;
    day_ctx.range.end = day_end;

    let result = schedule_items(&day_ctx, &request.tasks);
    let mut proposed = result.scheduled;
    let mut suggestions = result.suggestions;
    let unscheduled = result.unscheduled;

    if request.include_breaks {
        // Insert a 15-minute break between long adjacent focus proposals when gap is tiny.
        let mut with_breaks = Vec::new();
        let mut prev_end: Option<i64> = None;
        for block in &proposed {
            if let Some(pe) = prev_end {
                let gap = block.start - pe;
                if gap > 0 && gap < 20 * 60_000 {
                    // leave the buffer as-is; note suggestion
                    suggestions.push(Suggestion {
                        action: SuggestionAction::AddBreak,
                        message: format!(
                            "Consider a short break before \"{}\".",
                            block.title
                        ),
                        event_id: None,
                        start: Some(pe),
                        end: Some(block.start),
                    });
                }
            }
            with_breaks.push(block.clone());
            prev_end = Some(block.end);
        }
        proposed = with_breaks;
    }

    // Focus fragmentation check
    let occupancy = build_occupancy(&day_ctx);
    let meetings: Vec<_> = occupancy
        .iter()
        .filter(|b| b.source == BusySource::Event)
        .collect();
    let longest_free = find_free_slots(&day_ctx, 60 * 60_000, 1, None);
    if meetings.len() >= 4 && longest_free.is_empty() {
        suggestions.push(Suggestion {
            action: SuggestionAction::ProtectFocus,
            message: "Day is fragmented by meetings. Protect a focus block if possible.".into(),
            event_id: None,
            start: None,
            end: None,
        });
    }

    let capacity = compute_day_capacity(&day_ctx, request.day);
    if capacity.overloaded {
        suggestions.push(Suggestion {
            action: SuggestionAction::Redistribute,
            message: "Day looks overloaded — redistribute flexible events.".into(),
            event_id: None,
            start: None,
            end: None,
        });
    }

    PlanDayResult {
        date: capacity.date.clone(),
        proposed,
        capacity,
        suggestions,
        unscheduled,
    }
}

/// Suggest new times for movable events that conflict or to free focus time.
pub fn reschedule_flexible(
    ctx: &SchedulingContext,
    event: &Event,
) -> Result<Vec<FreeSlot>, String> {
    if !event.flexibility.is_movable() {
        return Err("Only Flexible or Optional events may be moved automatically.".into());
    }
    let duration = event.end_time - event.start_time;
    if duration <= 0 {
        return Err("Invalid event duration".into());
    }
    // Ensure current placement conflict check is informative.
    let _ = detect_conflicts(ctx, event.start_time, event.end_time, Some(&event.id));
    Ok(find_free_slots(ctx, duration, 5, Some(&event.id)))
}

#[cfg(test)]
mod deadline_deserialize_tests {
    use super::ScheduleItem;

    #[test]
    fn schedule_item_deadline_accepts_iso_string() {
        let item: ScheduleItem = serde_json::from_str(
            r#"{
                "title": "Climbing",
                "duration_minutes": 120,
                "deadline": "2026-08-01T19:00:00"
            }"#,
        )
        .expect("iso deadline should deserialize");
        assert!(item.deadline.is_some());
        assert!(item.deadline.unwrap() > 1_700_000_000_000);
    }

    #[test]
    fn schedule_item_deadline_accepts_millis() {
        let item: ScheduleItem = serde_json::from_str(
            r#"{
                "title": "Climbing",
                "duration_minutes": 120,
                "deadline": 1785600000000
            }"#,
        )
        .expect("millis deadline should deserialize");
        assert_eq!(item.deadline, Some(1785600000000));
    }
}
