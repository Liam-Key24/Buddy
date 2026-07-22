use crate::models::DateRange;
use serde::{Deserialize, Serialize};

pub const CONFLICT_KINDS: &[&str] = &[
    "overlap",
    "buffer_violation",
    "protected_sleep",
    "protected_work",
];

/// Policy knobs for the scheduling engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulingPolicy {
    /// Minutes of protected buffer before/after timed events.
    pub buffer_minutes: u32,
    pub protect_sleep: bool,
    pub protect_work: bool,
    /// Fraction of waking hours beyond which a day is overloaded (0–1).
    pub overload_threshold: f64,
    /// When true, free-time search may shrink buffer intervals.
    pub allow_reduce_buffer: bool,
}

impl Default for SchedulingPolicy {
    fn default() -> Self {
        Self {
            buffer_minutes: 10,
            protect_sleep: true,
            protect_work: true,
            overload_threshold: 0.70,
            allow_reduce_buffer: false,
        }
    }
}

impl SchedulingPolicy {
    pub fn buffer_ms(&self) -> i64 {
        self.buffer_minutes as i64 * 60_000
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeSlot {
    pub start: i64,
    pub end: i64,
    pub score: f64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayCapacity {
    pub date: String,
    pub booked_hours: f64,
    pub meeting_hours: f64,
    pub focus_hours: f64,
    pub free_hours: f64,
    pub waking_hours: f64,
    pub overloaded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionAction {
    MoveNew,
    KeepExisting,
    UseSlot,
    ProtectFocus,
    AddBreak,
    Redistribute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub action: SuggestionAction,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<i64>,
}

/// Reserved for tool/API request shapes.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindFreeTimeRequest {
    pub duration_minutes: u32,
    pub range: DateRange,
    #[serde(default)]
    pub limit: Option<usize>,
}
