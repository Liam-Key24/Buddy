use std::sync::Arc;

use buddy_core::{
    AfterExecute, AskKind, BuddyPlugin, FieldSpec, RespondMode, Safety, SettingSeed, Tool,
    ToolDecl, ToolSchema, ToolSpec,
};
use buddy_database::Database;
use serde_json::json;

/// Calendar AI tools are registered onto the tool registry from `AppState`
/// (they need `CalendarService`). This plugin contributes planner decls and settings seeds.
pub struct CalendarPlugin;

const DREAM_LOG_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "body",
        label: "dream description",
        required: true,
        memory_keys: &[],
    ask_kind: AskKind::Text,
    choices: &[],
    },
    FieldSpec {
        name: "sleep_date",
        label: "sleep date",
        required: false,
        memory_keys: &[],
    ask_kind: AskKind::Text,
    choices: &[],
    },
    FieldSpec {
        name: "title",
        label: "title",
        required: false,
        memory_keys: &[],
    ask_kind: AskKind::Text,
    choices: &[],
    },
];

const WORK_SALES_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "amount",
        label: "sales amount",
        required: true,
        memory_keys: &[],
    ask_kind: AskKind::Text,
    choices: &[],
    },
    FieldSpec {
        name: "currency",
        label: "currency",
        required: false,
        memory_keys: &["preferred_currency"],
    ask_kind: AskKind::Text,
    choices: &[],
    },
    FieldSpec {
        name: "work_date",
        label: "work date",
        required: false,
        memory_keys: &[],
    ask_kind: AskKind::Text,
    choices: &[],
    },
];

const WORK_HOURS_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "end_hm",
        label: "finish time",
        required: false,
        memory_keys: &[],
    ask_kind: AskKind::Text,
    choices: &[],
    },
    FieldSpec {
        name: "start_hm",
        label: "start time",
        required: false,
        memory_keys: &[],
    ask_kind: AskKind::Text,
    choices: &[],
    },
    FieldSpec {
        name: "actual_end_ms",
        label: "finish time",
        required: false,
        memory_keys: &[],
    ask_kind: AskKind::Text,
    choices: &[],
    },
    FieldSpec {
        name: "actual_start_ms",
        label: "start time",
        required: false,
        memory_keys: &[],
    ask_kind: AskKind::Text,
    choices: &[],
    },
];

const LOOK_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "when",
        label: "when",
        required: false,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
    FieldSpec {
        name: "focus",
        label: "focus",
        required: false,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
];

const PIN_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "title",
        label: "title",
        required: false,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
    FieldSpec {
        name: "start",
        label: "start time",
        required: false,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
];

const ORGANIZE_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "window",
        label: "window",
        required: false,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
    FieldSpec {
        name: "items",
        label: "items",
        required: false,
        memory_keys: &[],
        ask_kind: AskKind::Text,
        choices: &[],
    },
];

const CALENDAR_SCHEMAS: &[ToolSchema] = &[
    ToolSchema {
        tool: "calendar.look",
        fields: LOOK_FIELDS,
    },
    ToolSchema {
        tool: "calendar.pin",
        fields: PIN_FIELDS,
    },
    ToolSchema {
        tool: "calendar.organize",
        fields: ORGANIZE_FIELDS,
    },
    ToolSchema {
        tool: "dream.log",
        fields: DREAM_LOG_FIELDS,
    },
    ToolSchema {
        tool: "work.log_sales",
        fields: WORK_SALES_FIELDS,
    },
    ToolSchema {
        tool: "work.set_hours",
        fields: WORK_HOURS_FIELDS,
    },
];

fn extract_calendar_look(text: &str) -> Option<String> {
    let t = text.trim().to_ascii_lowercase();
    if t.is_empty() || t.contains(',') || t.contains(" and ") || t.contains(';') {
        return None;
    }
    let when = if t.contains("tomorrow") {
        "tomorrow"
    } else if t.contains("today") || t.contains("tonight") {
        "today"
    } else if t.contains("this week") {
        "this_week"
    } else if t.contains("next week") {
        "next_week"
    } else if t.contains("weekend") {
        "weekend"
    } else {
        return None;
    };
    let asks = t.contains("what")
        || t.contains("when")
        || t.contains("show")
        || t.contains("free")
        || t.contains("on");
    if !asks {
        return None;
    }
    let focus = if t.contains("free") {
        "free"
    } else if t.contains("working") || t.contains("work hours") {
        "work"
    } else {
        "events"
    };
    Some(json!({ "when": when, "focus": focus }).to_string())
}

const LOOK_SPEC: ToolSpec = ToolSpec {
    name: "calendar.look",
    description: "what's on / when free / when working",
    example: r#"calendar.look when=today focus=events"#,
    schema: ToolSchema {
        tool: "calendar.look",
        fields: LOOK_FIELDS,
    },
    aliases: &["/calendar.look"],
    rest_field: None,
    safety: Safety::Immediate,
    respond: RespondMode::Passthrough,
    likely: &["what's on", "whats on", "am i free", "when am i"],
    extract: Some(extract_calendar_look),
};

impl BuddyPlugin for CalendarPlugin {
    fn id(&self) -> &'static str {
        "calendar"
    }

    fn tools(&self, _db: Arc<Database>) -> Vec<Arc<dyn Tool>> {
        // Registered via buddy_calendar::register_calendar_tools in AppState.
        vec![]
    }

    fn tool_decls(&self) -> &'static [ToolDecl] {
        &[
            ToolDecl {
                name: "calendar.look",
                planner_line: "calendar.look: what's on / when free / when working. JSON: {\"when\":\"today|tomorrow|this_week|next_week|weekend|YYYY-MM-DD\", \"focus\":\"events|free|work\", \"duration_minutes\":60 optional}. focus=work for work hours (default this_week). Returns events plus Work/Sleep and off_days.",
            },
            ToolDecl {
                name: "calendar.pin",
                planner_line: "calendar.pin: fixed clock event. JSON: {\"action\":\"create|update|delete\", \"title\":\"Dentist\", \"start\":\"tomorrow 14:00\", \"end\":\"tomorrow 15:00\", \"id\":\"optional\"}. Use only with an explicit clock time.",
            },
            ToolDecl {
                name: "calendar.organize",
                planner_line: "calendar.organize: self-organize flexible time. JSON: {\"window\":\"this_week|next_week|today|tomorrow|weekend|sunday\", \"mode\":\"propose|commit\", \"constraints\":[\"after_work\"], \"items\":[{\"title\":\"Climbing\",\"duration\":\"90m\",\"when\":\"after_work\",\"count\":1}], \"lift_event_ids\":[]}. Always propose first. Empty items rebalances flexible events. If _ask tradeoff: move:<id> → re-call with lift_event_ids.",
            },
            ToolDecl {
                name: "lifestyle.list_blocks",
                planner_line: "lifestyle.list_blocks: list Work/Sleep schedule blocks in a range. tool_input JSON: {\"start\": <unix_ms>, \"end\": <unix_ms>}",
            },
            ToolDecl {
                name: "lifestyle.set_schedule",
                planner_line: "lifestyle.set_schedule: update permanent Work/Sleep hours. JSON: {\"kind\":\"work|sleep\",\"start_hm\":\"09:00\",\"end_hm\":\"17:00\"} or segments[].",
            },
            ToolDecl {
                name: "dream.log",
                planner_line: "dream.log: save a dream to last night's sleep (or sleep_date). tool_input JSON: {\"body\": \"...\", \"sleep_date\": \"optional YYYY-MM-DD\", \"title\": \"optional\", \"tags\": [\"optional\"]}",
            },
            ToolDecl {
                name: "dream.list",
                planner_line: "dream.list: list dreams for a sleep night. tool_input JSON: {\"sleep_date\": \"YYYY-MM-DD\"}",
            },
            ToolDecl {
                name: "dream.search",
                planner_line: "dream.search: search dreams by text/tags. tool_input JSON: {\"query\": \"nightmare\"}",
            },
            ToolDecl {
                name: "dream.update",
                planner_line: "dream.update: update a dream. tool_input JSON: {\"id\": \"...\", \"body\": \"optional\", \"title\": \"optional\", \"tags\": []}",
            },
            ToolDecl {
                name: "dream.delete",
                planner_line: "dream.delete: delete a dream by id. tool_input JSON: {\"id\": \"...\"}",
            },
            ToolDecl {
                name: "work.log_sales",
                planner_line: "work.log_sales: record sales for a work day. tool_input JSON: {\"amount\": 320, \"currency\": \"GBP\", \"work_date\": \"optional YYYY-MM-DD\"}",
            },
            ToolDecl {
                name: "work.set_hours",
                planner_line: "work.set_hours: override work start/end. tool_input JSON: {\"end_hm\": \"17:15\", \"start_hm\": \"optional\", \"work_date\": \"optional YYYY-MM-DD\"} or actual_start_ms/actual_end_ms",
            },
            ToolDecl {
                name: "work.get_stats",
                planner_line: "work.get_stats: hours and sales for today/week/month. tool_input may be empty JSON {}.",
            },
        ]
    }

    fn tool_schemas(&self) -> &'static [ToolSchema] {
        CALENDAR_SCHEMAS
    }

    fn tool_specs(&self) -> &'static [ToolSpec] {
        &[LOOK_SPEC]
    }

    fn setting_seeds(&self) -> &'static [SettingSeed] {
        &[
            SettingSeed {
                key: "calendar_notifications_enabled",
                value: "true",
            },
            SettingSeed {
                key: "calendar_default_timezone",
                value: "UTC",
            },
            SettingSeed {
                key: "calendar_default_reminders_json",
                value: "[{\"minutes_before\":15,\"method\":\"popup\"}]",
            },
            SettingSeed {
                key: "calendar_buffer_minutes",
                value: "10",
            },
            SettingSeed {
                key: "preferred_activity_duration",
                value: "",
            },
            SettingSeed {
                key: "preferred_meeting_time",
                value: "",
            },
            SettingSeed {
                key: "preferred_meeting_location",
                value: "",
            },
        ]
    }

    fn secret_keys(&self) -> &'static [&'static str] {
        &[]
    }

    fn after_execute_hint(&self, tool_name: &str) -> AfterExecute {
        match tool_name {
            "calendar.pin"
            | "calendar.organize"
            | "dream.log"
            | "dream.update"
            | "dream.delete"
            | "work.log_sales"
            | "work.set_hours"
            | "lifestyle.set_schedule" => AfterExecute::EmitCalendarUpdated,
            _ => AfterExecute::None,
        }
    }
}

impl CalendarPlugin {
    /// Single registration path for calendar/lifestyle tools (needs CalendarService).
    pub fn install(
        registry: &mut buddy_core::ToolRegistry,
        service: Arc<buddy_calendar::CalendarService>,
    ) {
        buddy_calendar::register_calendar_tools(registry, service);
    }
}

#[cfg(test)]
mod extract_tests {
    use super::extract_calendar_look;

    #[test]
    fn extract_look_today() {
        let raw = extract_calendar_look("what's on today?").unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["when"], "today");
        assert_eq!(v["focus"], "events");
        assert!(extract_calendar_look("tomorrow dentist at 10 and buy milk").is_none());
    }
}
