-- Runtime working state owned by Memory (pending clarification, active tasks).
-- Not user preferences — those stay in settings.

CREATE TABLE IF NOT EXISTS memory_runtime_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
