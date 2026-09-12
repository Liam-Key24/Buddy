use std::sync::Arc;

use buddy_core::{parse_tool_json, Tool, ToolError, ToolRegistry, ToolResult};
use chrono::{DateTime, NaiveDateTime};
use serde::Deserialize;
use serde_json::json;

use crate::models::{
    CreateEventInput, Flexibility, ScheduleKind, ScheduleSegment, UpdateEventInput,
};
use crate::scheduling::{
    infer_constraint_tokens, parse_local_datetime, OrganizeItemIn, OrganizeMode, ScheduleTradeoff,
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
            if let Some(ms) = parse_local_datetime(trimmed) {
                return Some(ms);
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

fn tradeoff_ask_value(tradeoff: &ScheduleTradeoff) -> serde_json::Value {
    json!({
        "field": "tradeoff",
        "prompt": tradeoff.prompt,
        "options": [
            {
                "id": "move",
                "label": format!(
                    "Move \"{}\", do \"{}\" in that slot",
                    tradeoff.blocking_title, tradeoff.new_title
                ),
                "value": format!("move:{}", tradeoff.blocking_event_id),
            },
            {
                "id": "keep",
                "label": format!(
                    "Keep \"{}\", find another time for \"{}\"",
                    tradeoff.blocking_title, tradeoff.new_title
                ),
                "value": "keep",
            },
            {
                "id": "later",
                "label": format!("Do \"{}\" later / another day", tradeoff.new_title),
                "value": "later",
            },
        ],
    })
}

fn attach_tradeoff_ask(
    mut value: serde_json::Value,
    tradeoff: Option<&ScheduleTradeoff>,
) -> serde_json::Value {
    if let Some(t) = tradeoff {
        if let Some(obj) = value.as_object_mut() {
            obj.insert("_ask".into(), tradeoff_ask_value(t));
        }
    }
    value
}

pub fn register_calendar_tools(registry: &mut ToolRegistry, service: Arc<CalendarService>) {
    for tool in make_calendar_tools(service) {
        registry.register(tool);
    }
}

pub fn make_calendar_tools(service: Arc<CalendarService>) -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(ListBlocksTool {
            service: service.clone(),
        }),
        Arc::new(SetScheduleTool {
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
        Arc::new(WorkGetStatsTool {
            service: service.clone(),
        }),
        Arc::new(LookTool {
            service: service.clone(),
        }),
        Arc::new(PinTool {
            service: service.clone(),
        }),
        Arc::new(OrganizeTool { service }),
    ]
}

#[derive(Debug, Deserialize)]
struct IdInput {
    id: String,
}

struct ListBlocksTool {
    service: Arc<CalendarService>,
}
struct SetScheduleTool {
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

#[derive(Debug, Deserialize)]
struct SetScheduleInput {
    kind: String,
    #[serde(default)]
    segments: Option<Vec<ScheduleSegment>>,
    #[serde(default)]
    start_hm: Option<String>,
    #[serde(default)]
    end_hm: Option<String>,
}

impl Tool for SetScheduleTool {
    fn name(&self) -> &str {
        "lifestyle.set_schedule"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: SetScheduleInput = parse_tool_json(input, "lifestyle.set_schedule")?;
        let kind = ScheduleKind::parse(&parsed.kind).ok_or_else(|| {
            ToolError::ExecutionFailed(format!("kind must be work or sleep, got {}", parsed.kind))
        })?;
        let rule = if let Some(segments) = parsed.segments.filter(|s| !s.is_empty()) {
            block_on(self.service.set_schedule_rule(kind, segments))?
        } else if let (Some(start), Some(end)) = (parsed.start_hm, parsed.end_hm) {
            block_on(self.service.set_schedule_times(kind, &start, &end))?
        } else {
            return Err(ToolError::ExecutionFailed(
                "provide segments or start_hm+end_hm".into(),
            ));
        };
        json_result(&rule)
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

struct LookTool {
    service: Arc<CalendarService>,
}
struct PinTool {
    service: Arc<CalendarService>,
}
struct OrganizeTool {
    service: Arc<CalendarService>,
}

#[derive(Debug, Deserialize)]
struct LookInput {
    #[serde(default)]
    when: Option<String>,
    #[serde(default)]
    focus: Option<String>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    duration_minutes: Option<u32>,
}

fn opt_instant<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(match v {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(s)) => Some(s),
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        Some(other) => Some(other.to_string()),
    })
}

#[derive(Debug, Deserialize)]
struct PinInput {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default, deserialize_with = "opt_instant")]
    start: Option<String>,
    #[serde(default, deserialize_with = "opt_instant")]
    end: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    force: bool,
}

#[derive(Debug, Deserialize, Default)]
struct OrganizeInput {
    #[serde(default)]
    window: Option<String>,
    #[serde(default)]
    items: Vec<OrganizeItemIn>,
    #[serde(default)]
    constraints: Vec<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    previous_items: Option<Vec<OrganizeItemIn>>,
    #[serde(default)]
    lift_event_ids: Option<Vec<String>>,
}

impl Tool for LookTool {
    fn name(&self) -> &str {
        "calendar.look"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: LookInput = if input.trim().is_empty() {
            LookInput {
                when: None,
                focus: None,
                query: None,
                duration_minutes: None,
            }
        } else {
            parse_tool_json(input, "calendar.look")?
        };
        let snap = block_on(self.service.look(
            parsed.when,
            parsed.focus,
            parsed.query,
            parsed.duration_minutes,
        ))?;
        json_result(&snap)
    }
}

impl Tool for PinTool {
    fn name(&self) -> &str {
        "calendar.pin"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: PinInput = parse_tool_json(input, "calendar.pin")?;
        let action = parsed
            .action
            .unwrap_or_else(|| "create".into())
            .to_ascii_lowercase();
        match action.as_str() {
            "delete" => {
                if let Some(id) = parsed.id {
                    block_on(self.service.delete_event(&id))?;
                    return json_result(&json!({ "deleted": id }));
                }
                if let Some(title) = parsed.title {
                    let ids = block_on(self.service.delete_events_matching(&title))?;
                    return json_result(&json!({ "deleted": ids }));
                }
                Err(ToolError::ExecutionFailed(
                    "pin delete needs id or title".into(),
                ))
            }
            "update" => {
                let id = parsed
                    .id
                    .ok_or_else(|| ToolError::ExecutionFailed("pin update needs id".into()))?;
                let start = parsed.start.as_deref().and_then(parse_local_datetime);
                let end = parsed.end.as_deref().and_then(parse_local_datetime);
                let outcome = block_on(self.service.update_event_checked(
                    &id,
                    UpdateEventInput {
                        title: parsed.title,
                        description: parsed.description,
                        location: parsed.location,
                        category: parsed.category,
                        color: None,
                        start_time: start,
                        end_time: end,
                        all_day: None,
                        timezone: None,
                        recurrence: None,
                        clear_recurrence: false,
                        reminders: None,
                        flexibility: Some(Flexibility::Fixed),
                        priority: None,
                        force: parsed.force,
                    },
                ))?;
                json_result(&outcome)
            }
            _ => {
                let title = parsed.title.unwrap_or_else(|| "Event".into());
                let start = parsed
                    .start
                    .as_deref()
                    .and_then(parse_local_datetime)
                    .ok_or_else(|| {
                        ToolError::ExecutionFailed("pin create needs start (e.g. tomorrow 14:00)".into())
                    })?;
                let end = parsed
                    .end
                    .as_deref()
                    .and_then(parse_local_datetime)
                    .unwrap_or(start + 60 * 60 * 1000);
                let outcome = block_on(self.service.create_event_checked(CreateEventInput {
                    title,
                    description: parsed.description,
                    location: parsed.location,
                    category: parsed.category,
                    color: None,
                    start_time: start,
                    end_time: end,
                    all_day: false,
                    timezone: None,
                    recurrence: None,
                    reminders: vec![],
                    flexibility: Some(Flexibility::Fixed),
                    priority: None,
                    force: parsed.force,
                }))?;
                json_result(&outcome)
            }
        }
    }
}

impl Tool for OrganizeTool {
    fn name(&self) -> &str {
        "calendar.organize"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: OrganizeInput = if input.trim().is_empty() {
            OrganizeInput::default()
        } else {
            parse_tool_json(input, "calendar.organize")?
        };
        let mut constraints = parsed.constraints;
        for token in infer_constraint_tokens(
            &format!(
                "{} {}",
                parsed.window.clone().unwrap_or_default(),
                constraints.join(" ")
            ),
        ) {
            if !constraints.iter().any(|c| c == &token) {
                constraints.push(token);
            }
        }
        let mode = match parsed.mode.as_deref().unwrap_or("propose") {
            "commit" | "apply" | "save" => OrganizeMode::Commit,
            _ => OrganizeMode::Propose,
        };
        let result = block_on(self.service.organize(
            parsed.window,
            parsed.items,
            constraints,
            mode,
            parsed.previous_items,
            parsed.lift_event_ids,
        ))?;
        let tradeoff = result.tradeoff.clone();
        let value = serde_json::to_value(&result)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        json_result(&attach_tradeoff_ask(value, tradeoff.as_ref()))
    }
}

