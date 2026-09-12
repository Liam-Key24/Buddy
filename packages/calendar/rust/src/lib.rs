//! Native BUDDY Calendar: local source-of-truth events, reminders, and AI tools.

mod error;
mod look_followup;
mod models;
mod notifications;
mod proposal;
pub mod scheduling;
mod services;
pub mod sync;
pub mod tools;

pub use error::CalendarError;
pub use look_followup::{
    last_look_from_tool_output, select_followup_payload, LastLook, LastLookBlock, LastLookEvent,
    LastLookSlot,
};
pub use proposal::{proposal_from_organize, GhostBlock, StoredCalendarProposal};
pub use models::{
    default_color_for_category, CreateDreamInput, CreateEventInput, DateRange, DreamEntry, Event,
    EventFilters, EventPriority, Flexibility, RecurrenceRule, Reminder, ReminderDelivery,
    LifestyleScheduleRule, ScheduleBlock, ScheduleKind, ScheduleSegment, UpdateDreamInput,
    UpdateEventInput, WorkDayLog, WorkStats,
    CATEGORIES,
};
pub use notifications::{
    dismiss_reminder, list_due_deliveries, list_notifications, mark_reminder_sent, snooze_reminder,
};
pub use scheduling::{
    ConflictReport, DayCapacity, DaySummary, FreeSlot, LookSnapshot, OrganizeItemIn, OrganizeMode,
    OrganizeResult, PlanDayRequest, PlanDayResult, ProposedBlock, ScheduleItem,
    ScheduleItemsResult, ScheduleTradeoff, SchedulingPolicy, WriteEventOutcome,
    expand_schedule_items, parse_duration_minutes, parse_local_datetime, parse_when_label,
    resolve_when_range, resolve_window,
};
pub use services::{month_buffer_range, CalendarService, SettingsLookup};
pub use tools::register_calendar_tools;
