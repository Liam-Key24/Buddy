use serde::{Deserialize, Serialize};

use crate::models::{Event, EventPriority, Flexibility};
use crate::scheduling::capacity::compute_day_capacity;
use crate::scheduling::conflict::detect_conflicts;
use crate::scheduling::free_time::{find_free_slots, find_free_slots_for};
use crate::scheduling::occupancy::{build_occupancy, BusySource};
use crate::scheduling::types::{DayCapacity, FreeSlot, Suggestion, SuggestionAction};
use crate::scheduling::{default_task_flexibility, default_task_priority, SchedulingContext};

/// Task/work item accepted by scheduling APIs (Calendar tools + future Tasks plugin).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleItem {
    pub title: String,
    pub duration_minutes: u32,
    #[serde(default)]
    pub deadline: Option<i64>,
    #[serde(default)]
    pub priority: Option<EventPriority>,
    #[serde(default)]
    pub flexibility: Option<Flexibility>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
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
pub struct ScheduleItemsResult {
    pub scheduled: Vec<ProposedBlock>,
    pub unscheduled: Vec<ScheduleItem>,
    pub suggestions: Vec<Suggestion>,
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

    let mut ordered: Vec<ScheduleItem> = items.to_vec();
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
        let slots = find_free_slots_for(
            &search_ctx,
            duration_ms,
            12,
            None,
            Some(item.title.as_str()),
        );
        let Some(best) = slots.into_iter().next() else {
            suggestions.push(Suggestion {
                action: SuggestionAction::Redistribute,
                message: format!(
                    "Could not find a free slot for \"{}\" ({} min) without violating protections.",
                    item.title, item.duration_minutes
                ),
                event_id: None,
                start: None,
                end: None,
            });
            unscheduled.push(item);
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
            let alt = find_free_slots_for(
                &search_ctx,
                duration_ms,
                16,
                None,
                Some(item.title.as_str()),
            )
            .into_iter()
            .find(|s| {
                let cap = compute_day_capacity(&working, s.start);
                !cap.overloaded
                    && (cap.booked_hours + added_hours) / cap.waking_hours.max(0.1)
                        < working.policy.overload_threshold
            });
            let Some(best) = alt else {
                suggestions.push(Suggestion {
                    action: SuggestionAction::Redistribute,
                    message: format!(
                        "Skipping \"{}\" to avoid overloading the day.",
                        item.title
                    ),
                    event_id: None,
                    start: None,
                    end: None,
                });
                unscheduled.push(item);
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
    }
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
    let slots = find_free_slots_for(ctx, duration_ms, 5, None, Some(title));
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
