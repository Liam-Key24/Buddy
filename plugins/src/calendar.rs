use std::sync::Arc;

use buddy_core::{AfterExecute, BuddyPlugin, FieldSpec, SettingSeed, Tool, ToolDecl, ToolSchema};
use buddy_database::Database;

/// Calendar AI tools are registered onto the tool registry from `AppState`
/// (they need `CalendarService`). This plugin contributes planner decls and settings seeds.
pub struct CalendarPlugin;

const CREATE_EVENT_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "title",
        label: "title",
        required: true,
        memory_keys: &[],
    },
    FieldSpec {
        name: "start_time",
        label: "date and time",
        required: true,
        memory_keys: &["preferred_meeting_time"],
    },
    FieldSpec {
        name: "end_time",
        label: "end time",
        required: true,
        memory_keys: &[],
    },
    FieldSpec {
        name: "location",
        label: "location",
        required: false,
        memory_keys: &["preferred_meeting_location", "default_meeting_location"],
    },
    FieldSpec {
        name: "description",
        label: "notes",
        required: false,
        memory_keys: &[],
    },
];

const DREAM_LOG_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "body",
        label: "dream description",
        required: true,
        memory_keys: &[],
    },
    FieldSpec {
        name: "sleep_date",
        label: "sleep date",
        required: false,
        memory_keys: &[],
    },
    FieldSpec {
        name: "title",
        label: "title",
        required: false,
        memory_keys: &[],
    },
];

const WORK_SALES_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "amount",
        label: "sales amount",
        required: true,
        memory_keys: &[],
    },
    FieldSpec {
        name: "currency",
        label: "currency",
        required: false,
        memory_keys: &["preferred_currency"],
    },
    FieldSpec {
        name: "work_date",
        label: "work date",
        required: false,
        memory_keys: &[],
    },
];

const WORK_HOURS_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "end_hm",
        label: "finish time",
        required: false,
        memory_keys: &[],
    },
    FieldSpec {
        name: "start_hm",
        label: "start time",
        required: false,
        memory_keys: &[],
    },
    FieldSpec {
        name: "actual_end_ms",
        label: "finish time",
        required: false,
        memory_keys: &[],
    },
    FieldSpec {
        name: "actual_start_ms",
        label: "start time",
        required: false,
        memory_keys: &[],
    },
];

const CALENDAR_SCHEMAS: &[ToolSchema] = &[
    ToolSchema {
        tool: "calendar.create_event",
        fields: CREATE_EVENT_FIELDS,
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
    ToolSchema {
        tool: "calendar.delete_event",
        fields: &[
            FieldSpec {
                name: "id",
                label: "event",
                required: false,
                memory_keys: &[],
            },
            FieldSpec {
                name: "query",
                label: "event name",
                required: false,
                memory_keys: &[],
            },
            FieldSpec {
                name: "all",
                label: "clear all",
                required: false,
                memory_keys: &[],
            },
        ],
    },
];

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
                name: "calendar.list_events",
                planner_line: "calendar.list_events: list BUDDY calendar events in a range. tool_input JSON: {\"start\": <unix_ms>, \"end\": <unix_ms>, \"query\": \"<optional>\", \"categories\": [\"work\"]}",
            },
            ToolDecl {
                name: "calendar.get_event",
                planner_line: "calendar.get_event: get one event by id. tool_input JSON: {\"id\": \"<event id>\"}",
            },
            ToolDecl {
                name: "calendar.create_event",
                planner_line: "calendar.create_event: create a native BUDDY calendar event. tool_input JSON: {\"title\": \"...\", \"start_time\": <unix_ms>, \"end_time\": <unix_ms>, \"description\": \"optional\", \"location\": \"optional\", \"category\": \"work|personal|birthdays|holidays|general\", \"all_day\": false, \"timezone\": \"optional IANA\", \"reminders\": [{\"minutes_before\":15}], \"recurrence\": {\"frequency\":\"WEEKLY\",\"interval\":1}, \"flexibility\":\"fixed|flexible|optional\", \"force\":false}. Returns status ok or conflict with suggestions.",
            },
            ToolDecl {
                name: "calendar.update_event",
                planner_line: "calendar.update_event: update an event. tool_input JSON: {\"id\": \"<event id>\", \"title\": \"optional\", \"start_time\": <optional unix_ms>, \"end_time\": <optional>, ...}",
            },
            ToolDecl {
                name: "calendar.delete_event",
                planner_line: "calendar.delete_event: delete event(s). tool_input JSON: {\"id\": \"<event id>\"} OR {\"query\": \"<title fragment>\"} OR {\"all\": true} to clear the calendar",
            },
            ToolDecl {
                name: "calendar.duplicate_event",
                planner_line: "calendar.duplicate_event: duplicate an event (shifted +1 day). tool_input JSON: {\"id\": \"<event id>\"}",
            },
            ToolDecl {
                name: "calendar.search_events",
                planner_line: "calendar.search_events: search events by text. tool_input JSON: {\"query\": \"...\", \"start\": <optional unix_ms>, \"end\": <optional unix_ms>}",
            },
            ToolDecl {
                name: "calendar.get_today",
                planner_line: "calendar.get_today: list today's events. tool_input may be empty JSON {}.",
            },
            ToolDecl {
                name: "calendar.get_tomorrow",
                planner_line: "calendar.get_tomorrow: list tomorrow's events. tool_input may be empty JSON {}.",
            },
            ToolDecl {
                name: "calendar.get_this_week",
                planner_line: "calendar.get_this_week: list this week's events. tool_input may be empty JSON {}.",
            },
            ToolDecl {
                name: "lifestyle.list_blocks",
                planner_line: "lifestyle.list_blocks: list Work/Sleep schedule blocks in a range. tool_input JSON: {\"start\": <unix_ms>, \"end\": <unix_ms>}",
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
            ToolDecl {
                name: "calendar.find_free_time",
                planner_line: "calendar.find_free_time: find ranked free slots respecting sleep/work/buffers. tool_input JSON: {\"duration_minutes\":120, \"start\": <unix_ms>, \"end\": <unix_ms>, \"limit\":5}",
            },
            ToolDecl {
                name: "calendar.block_time",
                planner_line: "calendar.block_time: smart focus time block. tool_input JSON: {\"title\":\"Coding\",\"duration_minutes\":180, \"start\": <optional>, \"end\": <optional>, \"apply\":true}",
            },
            ToolDecl {
                name: "calendar.schedule_task",
                planner_line: "calendar.schedule_task: auto-schedule a task or tasks[]. tool_input JSON: {\"title\":\"Design report\",\"duration_minutes\":120, \"deadline\": <unix_ms>, \"priority\":\"high\", \"flexibility\":\"flexible\", \"apply\":true} OR {\"tasks\":[...]}",
            },
            ToolDecl {
                name: "calendar.plan_day",
                planner_line: "calendar.plan_day: plan a day around protected blocks. tool_input JSON: {\"day\": <unix_ms>, \"tasks\":[{\"title\":\"...\",\"duration_minutes\":60}], \"include_breaks\":true, \"apply\":false}",
            },
            ToolDecl {
                name: "calendar.detect_conflicts",
                planner_line: "calendar.detect_conflicts: check a proposed window. tool_input JSON: {\"start\": <unix_ms>, \"end\": <unix_ms>, \"exclude_event_id\":\"optional\"}",
            },
            ToolDecl {
                name: "calendar.resolve_conflict",
                planner_line: "calendar.resolve_conflict: create event at a chosen resolution slot (force). tool_input JSON: {\"title\":\"...\", \"start\": <unix_ms>, \"end\": <unix_ms>}",
            },
            ToolDecl {
                name: "calendar.get_capacity",
                planner_line: "calendar.get_capacity: daily workload capacity. tool_input JSON: {\"day\": <unix_ms>} or {}",
            },
            ToolDecl {
                name: "calendar.day_summary",
                planner_line: "calendar.day_summary: intelligent daily summary + suggestions. tool_input JSON: {\"day\": <unix_ms>} or {}",
            },
        ]
    }

    fn tool_schemas(&self) -> &'static [ToolSchema] {
        CALENDAR_SCHEMAS
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
        ]
    }

    fn secret_keys(&self) -> &'static [&'static str] {
        &[]
    }

    fn after_execute_hint(&self, tool_name: &str) -> AfterExecute {
        match tool_name {
            "calendar.create_event"
            | "calendar.update_event"
            | "calendar.delete_event"
            | "calendar.duplicate_event"
            | "calendar.block_time"
            | "calendar.schedule_task"
            | "calendar.plan_day"
            | "calendar.resolve_conflict"
            | "dream.log"
            | "dream.update"
            | "dream.delete"
            | "work.log_sales"
            | "work.set_hours" => AfterExecute::EmitCalendarUpdated,
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
