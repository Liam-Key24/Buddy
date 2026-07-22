-- Native BUDDY Calendar: wipe legacy provider-cache table and create
-- the source-of-truth event store plus reminder delivery state.
DROP TABLE IF EXISTS calendar_reminder_states;
DROP TABLE IF EXISTS buddy_calendar_events;
DROP TABLE IF EXISTS calendar_events;

CREATE TABLE buddy_calendar_events (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    location TEXT,
    category TEXT NOT NULL DEFAULT 'general',
    color TEXT,
    start_time INTEGER NOT NULL,
    end_time INTEGER NOT NULL,
    all_day INTEGER NOT NULL DEFAULT 0,
    timezone TEXT NOT NULL DEFAULT 'UTC',
    recurrence_json TEXT,
    reminders_json TEXT NOT NULL DEFAULT '[]',
    external_provider TEXT,
    external_event_id TEXT,
    sync_status TEXT NOT NULL DEFAULT 'local',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_buddy_calendar_events_start
  ON buddy_calendar_events(start_time);
CREATE INDEX IF NOT EXISTS idx_buddy_calendar_events_end
  ON buddy_calendar_events(end_time);
CREATE INDEX IF NOT EXISTS idx_buddy_calendar_events_category
  ON buddy_calendar_events(category);
CREATE INDEX IF NOT EXISTS idx_buddy_calendar_events_external
  ON buddy_calendar_events(external_provider, external_event_id);

CREATE TABLE calendar_reminder_states (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL REFERENCES buddy_calendar_events(id) ON DELETE CASCADE,
    reminder_minutes INTEGER NOT NULL,
    fire_at INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    snoozed_until INTEGER,
    delivered_at INTEGER,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_calendar_reminder_fire_at
  ON calendar_reminder_states(fire_at);
CREATE INDEX IF NOT EXISTS idx_calendar_reminder_status
  ON calendar_reminder_states(status);
CREATE INDEX IF NOT EXISTS idx_calendar_reminder_event_id
  ON calendar_reminder_states(event_id);
