//! Native BUDDY Calendar: local source-of-truth events, reminders, and AI tools.

mod error;
mod models;
mod notifications;
mod services;
pub mod sync;
pub mod tools;

pub use error::CalendarError;
pub use models::{
    default_color_for_category, CreateDreamInput, CreateEventInput, DateRange, DreamEntry, Event,
    EventFilters, RecurrenceRule, Reminder, ReminderDelivery, ScheduleBlock, ScheduleKind,
    UpdateDreamInput, UpdateEventInput, WorkDayLog, WorkStats, CATEGORIES,
};
pub use notifications::{
    dismiss_reminder, list_due_deliveries, list_notifications, mark_reminder_sent, snooze_reminder,
};
pub use services::{month_buffer_range, CalendarService, SettingsLookup};
pub use tools::register_calendar_tools;
