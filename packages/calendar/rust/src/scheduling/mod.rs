//! AI scheduling types and engine for the Calendar plugin.
//!
//! Callers (AI tools, Tauri IPC, future Tasks plugin) should use these APIs
//! rather than re-implementing sleep/work/buffer rules.

mod capacity;
mod conflict;
mod free_time;
mod occupancy;
mod planner;
mod score;
mod snapshot;
mod summary;
mod types;
mod window;

pub use capacity::{compute_day_capacity, local_day_bounds_ms};
pub use conflict::{detect_conflicts, ConflictDetail, ConflictKind, ConflictReport};
pub use free_time::{find_free_slots, find_free_slots_for};
pub use occupancy::{build_occupancy, build_occupancy_excluding, BusyInterval, BusySource};
pub use planner::{
    block_focus_time, expand_schedule_items, plan_day, reschedule_flexible, schedule_items,
    OrganizeItemIn, OrganizeMode, OrganizeResult, PlanDayRequest, PlanDayResult, ProposedBlock,
    ScheduleItem, ScheduleItemsResult, ScheduleTradeoff,
};
pub use snapshot::{
    compose_look_snapshot, event_occupies_time, is_off_day_event, off_day_dates,
    punch_work_for_off_days, LookEvent, LookScheduleBlock, LookSnapshot, LookSlot,
};
pub(crate) use snapshot::look_block;
pub use score::{
    activity_hour_adjustment, score_slot, score_slot_for_activity, work_end_for_day,
};
pub use summary::{compose_day_summary, DaySummary, SummaryItem};
pub use types::{
    DayCapacity, FreeSlot, SchedulingPolicy, Suggestion, SuggestionAction, CONFLICT_KINDS,
};
pub use window::{
    constraint_after_work, constraint_weekend, infer_constraint_tokens, parse_duration_minutes,
    parse_local_datetime, parse_when_label, resolve_when_range, resolve_window,
};

use crate::models::{DateRange, Event, EventPriority, Flexibility, ScheduleBlock};
use serde::{Deserialize, Serialize};

/// Result of a conflict-aware create/update.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum WriteEventOutcome {
    Ok { event: Event },
    Conflict { report: ConflictReport },
}

/// Snapshot of calendar state used by pure scheduling helpers.
#[derive(Debug, Clone)]
pub struct SchedulingContext {
    pub events: Vec<Event>,
    pub lifestyle_blocks: Vec<ScheduleBlock>,
    pub policy: SchedulingPolicy,
    pub range: DateRange,
}

impl SchedulingContext {
    pub fn new(
        events: Vec<Event>,
        lifestyle_blocks: Vec<ScheduleBlock>,
        policy: SchedulingPolicy,
        range: DateRange,
    ) -> Self {
        Self {
            events,
            lifestyle_blocks,
            policy,
            range,
        }
    }
}

pub fn default_task_flexibility() -> Flexibility {
    Flexibility::Flexible
}

pub fn default_task_priority() -> EventPriority {
    EventPriority::Normal
}
