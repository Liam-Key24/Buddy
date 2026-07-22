# BUDDY Calendar

Native BUDDY Calendar — local source of truth for events and reminders.

## Layout

```
packages/calendar/
  models/          Shared TS event + notification types
  services/        Pure helpers (merge, day filters)
  utils/           Dates + category colours
  hooks/           View/search helpers
  notifications/   Reminder settings + snooze options
  api/             IPC contract types
  ui/              Shared UI exports (desktop UI lives in the app)
  rust/            buddy-calendar crate (CalendarService)
  tests/           Vitest unit tests
```

## Architecture

- **BUDDY Calendar** owns events in SQLite (`buddy_calendar_events`).
- Reminders are tracked in `calendar_reminder_states` and delivered by the Tauri reminder loop.
- Future sync providers implement `SyncProvider` in `rust/src/sync/` without changing core CRUD.

## Settings keys

- `calendar_notifications_enabled`
- `calendar_default_timezone`
- `calendar_default_reminders_json`

## Tests

```bash
cargo test -p buddy-calendar
cargo test -p buddy-plugins --test calendar_tools
cd app && npm test
```
