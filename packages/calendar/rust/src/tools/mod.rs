use std::sync::Arc;

use buddy_core::{parse_tool_json, Tool, ToolError, ToolRegistry, ToolResult};
use chrono::{DateTime, NaiveDateTime};
use serde::Deserialize;
use serde_json::json;

use crate::models::{
    CreateEventInput, DateRange, EventFilters, RecurrenceRule, Reminder, UpdateEventInput,
};
use crate::CalendarService;

/// Accept unix ms (number/string) or common ISO-8601 datetime strings.
fn parse_millis_value(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if let Ok(n) = trimmed.parse::<i64>() {
                return Some(n);
            }
            if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
                return Some(dt.timestamp_millis());
            }
            if let Ok(dt) = DateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S%.f%z") {
                return Some(dt.timestamp_millis());
            }
            if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S") {
                return Some(naive.and_utc().timestamp_millis());
            }
            if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
                return Some(naive.and_utc().timestamp_millis());
            }
            if let Ok(naive) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M") {
                return Some(naive.and_utc().timestamp_millis());
            }
            None
        }
        _ => None,
    }
}

fn deserialize_millis<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    parse_millis_value(&value).ok_or_else(|| {
        serde::de::Error::custom(format!("expected unix ms or ISO datetime, got {value}"))
    })
}

fn deserialize_opt_millis<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(v) => parse_millis_value(&v)
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid datetime: {v}"))),
    }
}

fn block_on<F, T>(fut: F) -> Result<T, ToolError>
where
    F: std::future::Future<Output = Result<T, crate::CalendarError>>,
{
    let result = match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(fut)),
        Err(_) => tokio::runtime::Runtime::new()
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            .block_on(fut),
    };
    result.map_err(|e| ToolError::ExecutionFailed(format!("{}: {}", e.code(), e)))
}

fn json_result<T: serde::Serialize>(value: &T) -> Result<ToolResult, ToolError> {
    let output = serde_json::to_string_pretty(value)
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
    Ok(ToolResult { output })
}

pub fn register_calendar_tools(registry: &mut ToolRegistry, service: Arc<CalendarService>) {
    for tool in make_calendar_tools(service) {
        registry.register(tool);
    }
}

pub fn make_calendar_tools(service: Arc<CalendarService>) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(ListEventsTool {
            service: service.clone(),
        }),
        Arc::new(GetEventTool {
            service: service.clone(),
        }),
        Arc::new(CreateEventTool {
            service: service.clone(),
        }),
        Arc::new(UpdateEventTool {
            service: service.clone(),
        }),
        Arc::new(DeleteEventTool {
            service: service.clone(),
        }),
        Arc::new(DuplicateEventTool {
            service: service.clone(),
        }),
        Arc::new(SearchEventsTool {
            service: service.clone(),
        }),
        Arc::new(GetTodayTool {
            service: service.clone(),
        }),
        Arc::new(GetTomorrowTool {
            service: service.clone(),
        }),
        Arc::new(GetThisWeekTool {
            service: service.clone(),
        }),
        Arc::new(ListBlocksTool {
            service: service.clone(),
        }),
        Arc::new(DreamLogTool {
            service: service.clone(),
        }),
        Arc::new(DreamListTool {
            service: service.clone(),
        }),
        Arc::new(DreamSearchTool {
            service: service.clone(),
        }),
        Arc::new(DreamUpdateTool {
            service: service.clone(),
        }),
        Arc::new(DreamDeleteTool {
            service: service.clone(),
        }),
        Arc::new(WorkLogSalesTool {
            service: service.clone(),
        }),
        Arc::new(WorkSetHoursTool {
            service: service.clone(),
        }),
        Arc::new(WorkGetStatsTool { service }),
    ]
}

struct ListEventsTool {
    service: Arc<CalendarService>,
}
struct GetEventTool {
    service: Arc<CalendarService>,
}
struct CreateEventTool {
    service: Arc<CalendarService>,
}
struct UpdateEventTool {
    service: Arc<CalendarService>,
}
struct DeleteEventTool {
    service: Arc<CalendarService>,
}
struct DuplicateEventTool {
    service: Arc<CalendarService>,
}
struct SearchEventsTool {
    service: Arc<CalendarService>,
}
struct GetTodayTool {
    service: Arc<CalendarService>,
}
struct GetTomorrowTool {
    service: Arc<CalendarService>,
}
struct GetThisWeekTool {
    service: Arc<CalendarService>,
}

#[derive(Debug, Deserialize)]
struct ListEventsInput {
    #[serde(alias = "start_time", deserialize_with = "deserialize_millis")]
    start: i64,
    #[serde(alias = "end_time", deserialize_with = "deserialize_millis")]
    end: i64,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GetEventInput {
    id: String,
}

#[derive(Debug, Deserialize)]
struct CreateInput {
    title: String,
    #[serde(alias = "start", deserialize_with = "deserialize_millis")]
    start_time: i64,
    #[serde(alias = "end", deserialize_with = "deserialize_millis")]
    end_time: i64,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    all_day: bool,
    #[serde(default)]
    timezone: Option<String>,
    #[serde(default)]
    recurrence: Option<RecurrenceRule>,
    #[serde(default)]
    reminders: Vec<Reminder>,
}

#[derive(Debug, Deserialize)]
struct UpdateInput {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default, alias = "start", deserialize_with = "deserialize_opt_millis")]
    start_time: Option<i64>,
    #[serde(default, alias = "end", deserialize_with = "deserialize_opt_millis")]
    end_time: Option<i64>,
    #[serde(default)]
    all_day: Option<bool>,
    #[serde(default)]
    timezone: Option<String>,
    #[serde(default)]
    recurrence: Option<RecurrenceRule>,
    #[serde(default)]
    clear_recurrence: bool,
    #[serde(default)]
    reminders: Option<Vec<Reminder>>,
}

#[derive(Debug, Deserialize)]
struct IdInput {
    id: String,
}

#[derive(Debug, Deserialize)]
struct DeleteInput {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    all: bool,
    #[serde(default, alias = "title")]
    query: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchInput {
    query: String,
    #[serde(default, alias = "start_time", deserialize_with = "deserialize_opt_millis")]
    start: Option<i64>,
    #[serde(default, alias = "end_time", deserialize_with = "deserialize_opt_millis")]
    end: Option<i64>,
}

impl Tool for ListEventsTool {
    fn name(&self) -> &str {
        "calendar.list_events"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: ListEventsInput = parse_tool_json(input, "calendar.list_events")?;
        let events = block_on(self.service.list_events(
            DateRange {
                start: parsed.start,
                end: parsed.end,
            },
            EventFilters {
                query: parsed.query,
                categories: parsed.categories,
            },
        ))?;
        json_result(&events)
    }
}

impl Tool for GetEventTool {
    fn name(&self) -> &str {
        "calendar.get_event"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: GetEventInput = parse_tool_json(input, "calendar.get_event")?;
        let event = block_on(self.service.get_event(&parsed.id))?;
        json_result(&event)
    }
}

impl Tool for CreateEventTool {
    fn name(&self) -> &str {
        "calendar.create_event"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        // Support batch: {"events":[...]} from multi-activity schedule parses.
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(input) {
            if let Some(arr) = value.get("events").and_then(|e| e.as_array()) {
                let mut created = Vec::new();
                for item in arr {
                    let parsed: CreateInput = serde_json::from_value(item.clone())
                        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                    let event = block_on(self.service.create_event(CreateEventInput {
                        title: parsed.title,
                        description: parsed.description,
                        location: parsed.location,
                        category: parsed.category,
                        color: parsed.color,
                        start_time: parsed.start_time,
                        end_time: parsed.end_time,
                        all_day: parsed.all_day,
                        timezone: parsed.timezone,
                        recurrence: parsed.recurrence,
                        reminders: parsed.reminders,
                    }))?;
                    created.push(event);
                }
                return json_result(&created);
            }
        }

        let parsed: CreateInput = parse_tool_json(input, "calendar.create_event")?;
        let event = block_on(self.service.create_event(CreateEventInput {
            title: parsed.title,
            description: parsed.description,
            location: parsed.location,
            category: parsed.category,
            color: parsed.color,
            start_time: parsed.start_time,
            end_time: parsed.end_time,
            all_day: parsed.all_day,
            timezone: parsed.timezone,
            recurrence: parsed.recurrence,
            reminders: parsed.reminders,
        }))?;
        json_result(&event)
    }
}

impl Tool for UpdateEventTool {
    fn name(&self) -> &str {
        "calendar.update_event"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: UpdateInput = parse_tool_json(input, "calendar.update_event")?;
        let event = block_on(self.service.update_event(
            &parsed.id,
            UpdateEventInput {
                title: parsed.title,
                description: parsed.description,
                location: parsed.location,
                category: parsed.category,
                color: parsed.color,
                start_time: parsed.start_time,
                end_time: parsed.end_time,
                all_day: parsed.all_day,
                timezone: parsed.timezone,
                recurrence: parsed.recurrence,
                clear_recurrence: parsed.clear_recurrence,
                reminders: parsed.reminders,
            },
        ))?;
        json_result(&event)
    }
}

impl Tool for DeleteEventTool {
    fn name(&self) -> &str {
        "calendar.delete_event"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: DeleteInput = parse_tool_json(input, "calendar.delete_event")?;
        if parsed.all {
            let count = block_on(self.service.delete_all_events())?;
            return Ok(ToolResult {
                output: json!({"deleted": true, "all": true, "count": count}).to_string(),
            });
        }
        if let Some(id) = parsed.id.filter(|s| !s.trim().is_empty()) {
            block_on(self.service.delete_event(&id))?;
            return Ok(ToolResult {
                output: json!({"deleted": true, "id": id}).to_string(),
            });
        }
        if let Some(query) = parsed.query.filter(|s| !s.trim().is_empty()) {
            let deleted_ids = block_on(self.service.delete_events_matching(&query))?;
            if deleted_ids.is_empty() {
                return Err(ToolError::ExecutionFailed(format!(
                    "no events matched query '{query}'"
                )));
            }
            return Ok(ToolResult {
                output: json!({
                    "deleted": true,
                    "query": query,
                    "ids": deleted_ids,
                    "count": deleted_ids.len()
                })
                .to_string(),
            });
        }
        Err(ToolError::ExecutionFailed(
            "calendar.delete_event needs id, query/title, or all=true".into(),
        ))
    }
}

impl Tool for DuplicateEventTool {
    fn name(&self) -> &str {
        "calendar.duplicate_event"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: IdInput = parse_tool_json(input, "calendar.duplicate_event")?;
        let event = block_on(self.service.duplicate_event(&parsed.id))?;
        json_result(&event)
    }
}

impl Tool for SearchEventsTool {
    fn name(&self) -> &str {
        "calendar.search_events"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: SearchInput = parse_tool_json(input, "calendar.search_events")?;
        let range = match (parsed.start, parsed.end) {
            (Some(start), Some(end)) => Some(DateRange { start, end }),
            _ => None,
        };
        let events = block_on(self.service.search_events(&parsed.query, range))?;
        json_result(&events)
    }
}

impl Tool for GetTodayTool {
    fn name(&self) -> &str {
        "calendar.get_today"
    }
    fn execute(&self, _input: &str) -> Result<ToolResult, ToolError> {
        let events = block_on(self.service.get_today())?;
        json_result(&events)
    }
}

impl Tool for GetTomorrowTool {
    fn name(&self) -> &str {
        "calendar.get_tomorrow"
    }
    fn execute(&self, _input: &str) -> Result<ToolResult, ToolError> {
        let events = block_on(self.service.get_tomorrow())?;
        json_result(&events)
    }
}

impl Tool for GetThisWeekTool {
    fn name(&self) -> &str {
        "calendar.get_this_week"
    }
    fn execute(&self, _input: &str) -> Result<ToolResult, ToolError> {
        let events = block_on(self.service.get_this_week())?;
        json_result(&events)
    }
}

struct ListBlocksTool {
    service: Arc<CalendarService>,
}
struct DreamLogTool {
    service: Arc<CalendarService>,
}
struct DreamListTool {
    service: Arc<CalendarService>,
}
struct DreamSearchTool {
    service: Arc<CalendarService>,
}
struct DreamUpdateTool {
    service: Arc<CalendarService>,
}
struct DreamDeleteTool {
    service: Arc<CalendarService>,
}
struct WorkLogSalesTool {
    service: Arc<CalendarService>,
}
struct WorkSetHoursTool {
    service: Arc<CalendarService>,
}
struct WorkGetStatsTool {
    service: Arc<CalendarService>,
}

#[derive(Debug, Deserialize)]
struct ListBlocksInput {
    #[serde(alias = "start_time", deserialize_with = "deserialize_millis")]
    start: i64,
    #[serde(alias = "end_time", deserialize_with = "deserialize_millis")]
    end: i64,
}

#[derive(Debug, Deserialize)]
struct DreamLogInput {
    body: String,
    #[serde(default)]
    sleep_date: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DreamListInput {
    sleep_date: String,
}

#[derive(Debug, Deserialize)]
struct DreamSearchInput {
    query: String,
}

#[derive(Debug, Deserialize)]
struct DreamUpdateInput {
    id: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct WorkSalesInput {
    amount: f64,
    #[serde(default)]
    work_date: Option<String>,
    #[serde(default)]
    currency: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WorkHoursInput {
    #[serde(default)]
    work_date: Option<String>,
    #[serde(default)]
    actual_start_ms: Option<i64>,
    #[serde(default)]
    actual_end_ms: Option<i64>,
    /// Wall-clock "17:15" or "5:15pm" for today's (or work_date) end.
    #[serde(default)]
    end_hm: Option<String>,
    #[serde(default)]
    start_hm: Option<String>,
}

fn parse_hm_to_ms(work_date: &str, hm: &str) -> Result<i64, ToolError> {
    let lower = hm.trim().to_lowercase();
    let re = regex_lite_hm(&lower)?;
    crate::services::schedule_service::set_time_on_date(work_date, re.0, re.1)
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))
}

fn regex_lite_hm(lower: &str) -> Result<(u32, u32), ToolError> {
    let cleaned = lower.replace(' ', "");
    let (num, ampm) = if cleaned.ends_with("pm") {
        (&cleaned[..cleaned.len() - 2], Some("pm"))
    } else if cleaned.ends_with("am") {
        (&cleaned[..cleaned.len() - 2], Some("am"))
    } else {
        (cleaned.as_str(), None)
    };
    let parts: Vec<_> = num.split(':').collect();
    let hour: u32 = parts
        .first()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| ToolError::ExecutionFailed(format!("bad time {lower}")))?;
    let minute: u32 = if parts.len() > 1 {
        parts[1]
            .parse()
            .map_err(|_| ToolError::ExecutionFailed(format!("bad time {lower}")))?
    } else {
        0
    };
    let mut h = hour;
    if ampm == Some("pm") && h < 12 {
        h += 12;
    } else if ampm == Some("am") && h == 12 {
        h = 0;
    }
    Ok((h % 24, minute))
}

impl Tool for ListBlocksTool {
    fn name(&self) -> &str {
        "lifestyle.list_blocks"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: ListBlocksInput = parse_tool_json(input, "lifestyle.list_blocks")?;
        let blocks = block_on(self.service.list_schedule_blocks(parsed.start, parsed.end))?;
        json_result(&blocks)
    }
}

impl Tool for DreamLogTool {
    fn name(&self) -> &str {
        "dream.log"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: DreamLogInput = parse_tool_json(input, "dream.log")?;
        let dream = block_on(self.service.log_dream(crate::models::CreateDreamInput {
            body: parsed.body,
            sleep_date: parsed.sleep_date,
            title: parsed.title,
            tags: parsed.tags,
            mood: None,
            sleep_quality: None,
        }))?;
        json_result(&dream)
    }
}

impl Tool for DreamListTool {
    fn name(&self) -> &str {
        "dream.list"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: DreamListInput = parse_tool_json(input, "dream.list")?;
        let dreams = block_on(self.service.list_dreams(&parsed.sleep_date))?;
        json_result(&dreams)
    }
}

impl Tool for DreamSearchTool {
    fn name(&self) -> &str {
        "dream.search"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: DreamSearchInput = parse_tool_json(input, "dream.search")?;
        let dreams = block_on(self.service.search_dreams(&parsed.query))?;
        json_result(&dreams)
    }
}

impl Tool for DreamUpdateTool {
    fn name(&self) -> &str {
        "dream.update"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: DreamUpdateInput = parse_tool_json(input, "dream.update")?;
        let dream = block_on(self.service.update_dream(
            &parsed.id,
            crate::models::UpdateDreamInput {
                body: parsed.body,
                title: parsed.title,
                tags: parsed.tags,
                mood: None,
                sleep_quality: None,
            },
        ))?;
        json_result(&dream)
    }
}

impl Tool for DreamDeleteTool {
    fn name(&self) -> &str {
        "dream.delete"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: IdInput = parse_tool_json(input, "dream.delete")?;
        block_on(self.service.delete_dream(&parsed.id))?;
        Ok(ToolResult {
            output: json!({"deleted": true, "id": parsed.id}).to_string(),
        })
    }
}

impl Tool for WorkLogSalesTool {
    fn name(&self) -> &str {
        "work.log_sales"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: WorkSalesInput = parse_tool_json(input, "work.log_sales")?;
        let log = block_on(self.service.log_work_sales(
            parsed.work_date,
            parsed.amount,
            parsed.currency,
        ))?;
        json_result(&log)
    }
}

impl Tool for WorkSetHoursTool {
    fn name(&self) -> &str {
        "work.set_hours"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: WorkHoursInput = parse_tool_json(input, "work.set_hours")?;
        let date = parsed
            .work_date
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(crate::services::schedule_service::today_date_string);
        let mut start_ms = parsed.actual_start_ms;
        let mut end_ms = parsed.actual_end_ms;
        if let Some(hm) = parsed.start_hm.as_deref() {
            start_ms = Some(parse_hm_to_ms(&date, hm)?);
        }
        if let Some(hm) = parsed.end_hm.as_deref() {
            end_ms = Some(parse_hm_to_ms(&date, hm)?);
        }
        let log = block_on(self.service.set_work_hours(Some(date), start_ms, end_ms))?;
        json_result(&log)
    }
}

impl Tool for WorkGetStatsTool {
    fn name(&self) -> &str {
        "work.get_stats"
    }
    fn execute(&self, _input: &str) -> Result<ToolResult, ToolError> {
        let stats = block_on(self.service.get_work_stats())?;
        json_result(&stats)
    }
}
