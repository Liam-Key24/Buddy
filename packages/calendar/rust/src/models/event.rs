use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecurrenceRule {
    /// RRULE-style frequency: DAILY, WEEKLY, MONTHLY, YEARLY
    pub frequency: String,
    #[serde(default = "default_interval")]
    pub interval: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub by_day: Vec<String>,
}

fn default_interval() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reminder {
    /// Minutes before start
    pub minutes_before: u32,
    #[serde(default = "default_reminder_method")]
    pub method: String,
}

fn default_reminder_method() -> String {
    "popup".to_string()
}

/// Scheduling flexibility for an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Flexibility {
    #[default]
    Fixed,
    Flexible,
    Optional,
}

impl Flexibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Flexible => "flexible",
            Self::Optional => "optional",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "flexible" => Self::Flexible,
            "optional" => Self::Optional,
            _ => Self::Fixed,
        }
    }

    pub fn is_movable(self) -> bool {
        matches!(self, Self::Flexible | Self::Optional)
    }
}

/// Event / task priority for scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EventPriority {
    Low,
    #[default]
    Normal,
    High,
}

impl EventPriority {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "low" => Self::Low,
            "high" => Self::High,
            _ => Self::Normal,
        }
    }

    pub fn rank(self) -> i32 {
        match self {
            Self::High => 3,
            Self::Normal => 2,
            Self::Low => 1,
        }
    }
}

/// Canonical BUDDY calendar event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Unix milliseconds (UTC)
    pub start_time: i64,
    pub end_time: i64,
    pub all_day: bool,
    pub timezone: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<RecurrenceRule>,
    pub reminders: Vec<Reminder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_event_id: Option<String>,
    pub sync_status: String,
    pub created_at: i64,
    pub updated_at: i64,
    /// Set when this is an expanded occurrence of a recurring master.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurrence_of: Option<String>,
    #[serde(default)]
    pub flexibility: Flexibility,
    #[serde(default)]
    pub priority: EventPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEventInput {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    pub start_time: i64,
    pub end_time: i64,
    #[serde(default)]
    pub all_day: bool,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub recurrence: Option<RecurrenceRule>,
    #[serde(default)]
    pub reminders: Vec<Reminder>,
    #[serde(default)]
    pub flexibility: Option<Flexibility>,
    #[serde(default)]
    pub priority: Option<EventPriority>,
    /// Skip conflict checks and write anyway.
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEventInput {
    #[serde(default)]
    pub title: Option<String>,
    /// Empty string clears the field.
    #[serde(default)]
    pub description: Option<String>,
    /// Empty string clears the field.
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    /// Empty string clears the field.
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub start_time: Option<i64>,
    #[serde(default)]
    pub end_time: Option<i64>,
    #[serde(default)]
    pub all_day: Option<bool>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub recurrence: Option<RecurrenceRule>,
    #[serde(default)]
    pub clear_recurrence: bool,
    #[serde(default)]
    pub reminders: Option<Vec<Reminder>>,
    #[serde(default)]
    pub flexibility: Option<Flexibility>,
    #[serde(default)]
    pub priority: Option<EventPriority>,
    /// Skip conflict checks and write anyway.
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DateRange {
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventFilters {
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderDelivery {
    pub id: String,
    pub event_id: String,
    pub event_title: String,
    pub reminder_minutes: i64,
    pub fire_at: i64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snoozed_until: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivered_at: Option<i64>,
}

pub const CATEGORIES: &[(&str, &str, &str)] = &[
    ("work", "Work", "#3B82F6"),
    ("personal", "Personal", "#8B5CF6"),
    ("birthdays", "Birthdays", "#10B981"),
    ("holidays", "Holidays", "#F59E0B"),
    ("general", "General", "#64748B"),
];

pub fn default_color_for_category(category: &str) -> &'static str {
    CATEGORIES
        .iter()
        .find(|(id, _, _)| *id == category)
        .map(|(_, _, color)| *color)
        .unwrap_or("#64748B")
}
