CREATE TABLE IF NOT EXISTS research_sessions (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL DEFAULT 'Research',
    question TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
    findings_json TEXT NOT NULL DEFAULT '[]',
    sources_json TEXT NOT NULL DEFAULT '[]',
    details TEXT NOT NULL DEFAULT '',
    open_questions TEXT NOT NULL DEFAULT '',
    next_steps TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_research_updated ON research_sessions(updated_at);
