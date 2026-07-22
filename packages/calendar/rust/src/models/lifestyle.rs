use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScheduleKind {
    Work,
    Sleep,
}

impl ScheduleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Sleep => "sleep",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "work" => Some(Self::Work),
            "sleep" => Some(Self::Sleep),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleSegment {
    pub by_day: Vec<String>,
    pub start_hm: String,
    pub end_hm: String,
    #[serde(default)]
    pub crosses_midnight: bool,
}

/// Expanded lifestyle block for a visible range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleBlock {
    /// Synthetic id: `work::{YYYY-MM-DD}` or `sleep::{YYYY-MM-DD}` (sleep start date).
    pub id: String,
    pub kind: ScheduleKind,
    pub title: String,
    /// Unix ms
    pub start_time: i64,
    pub end_time: i64,
    /// YYYY-MM-DD for the block's primary date (work day or sleep start night).
    pub anchor_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamEntry {
    pub id: String,
    pub sleep_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub body: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mood: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sleep_quality: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDreamInput {
    pub body: String,
    #[serde(default)]
    pub sleep_date: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub mood: Option<i64>,
    #[serde(default)]
    pub sleep_quality: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDreamInput {
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub mood: Option<i64>,
    #[serde(default)]
    pub sleep_quality: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkDayLog {
    pub work_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_start_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_end_ms: Option<i64>,
    pub sales_amount: f64,
    pub sales_currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkPeriodStats {
    pub hours: f64,
    pub sales: f64,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkStats {
    pub today: WorkPeriodStats,
    pub week: WorkPeriodStats,
    pub month: WorkPeriodStats,
}
