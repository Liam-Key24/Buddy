-- Calendar scheduling: local mirror of proposed/scheduled events plus the
-- policy metadata Buddy needs to prioritise and reschedule around conflicts.
-- The provider (self-hosted cal.com) owns the real bookings; this table keeps
-- Buddy's source of truth for intelligence and conflict resolution.
CREATE TABLE IF NOT EXISTS calendar_events (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    source_type TEXT NOT NULL DEFAULT 'manual',
    source_id TEXT,
    start_at INTEGER NOT NULL,
    end_at INTEGER NOT NULL,
    timezone TEXT NOT NULL DEFAULT 'UTC',
    priority TEXT NOT NULL DEFAULT 'normal',
    movable INTEGER NOT NULL DEFAULT 1,
    confidence REAL NOT NULL DEFAULT 1.0,
    kind TEXT NOT NULL DEFAULT 'focus',
    status TEXT NOT NULL DEFAULT 'proposed',
    provider TEXT,
    provider_event_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_calendar_events_start_at ON calendar_events(start_at);
CREATE INDEX IF NOT EXISTS idx_calendar_events_status ON calendar_events(status);
CREATE INDEX IF NOT EXISTS idx_calendar_events_provider_event_id
  ON calendar_events(provider_event_id);
