-- Lifestyle schedule layers (Work / Sleep), dream log, and work day logs.
-- These are NOT normal calendar events and do not use reminder_states.

CREATE TABLE IF NOT EXISTS lifestyle_schedule_rules (
    kind TEXT PRIMARY KEY CHECK (kind IN ('work', 'sleep')),
    segments_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS dream_entries (
    id TEXT PRIMARY KEY,
    sleep_date TEXT NOT NULL,
    title TEXT,
    body TEXT NOT NULL,
    tags_json TEXT NOT NULL DEFAULT '[]',
    mood INTEGER,
    sleep_quality INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_dream_entries_sleep_date
  ON dream_entries(sleep_date);

CREATE TABLE IF NOT EXISTS work_day_logs (
    work_date TEXT PRIMARY KEY,
    actual_start_ms INTEGER,
    actual_end_ms INTEGER,
    sales_amount REAL NOT NULL DEFAULT 0,
    sales_currency TEXT NOT NULL DEFAULT 'GBP',
    notes TEXT,
    updated_at INTEGER NOT NULL
);

-- Seed default personal schedule (local wall-clock times).
INSERT OR IGNORE INTO lifestyle_schedule_rules (kind, segments_json, updated_at) VALUES (
  'sleep',
  '[{"by_day":["MO","TU","WE","TH"],"start_hm":"22:30","end_hm":"07:45","crosses_midnight":true},{"by_day":["FR"],"start_hm":"23:00","end_hm":"07:45","crosses_midnight":true},{"by_day":["SA"],"start_hm":"23:00","end_hm":"08:30","crosses_midnight":true},{"by_day":["SU"],"start_hm":"22:30","end_hm":"07:45","crosses_midnight":true}]',
  0
);

INSERT OR IGNORE INTO lifestyle_schedule_rules (kind, segments_json, updated_at) VALUES (
  'work',
  '[{"by_day":["MO","TU","WE","TH","FR"],"start_hm":"08:45","end_hm":"16:45","crosses_midnight":false}]',
  0
);
