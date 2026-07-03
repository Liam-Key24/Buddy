CREATE TABLE IF NOT EXISTS sparks (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    tags TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_nudged_at INTEGER,
    source_conversation_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_sparks_status ON sparks(status);
CREATE INDEX IF NOT EXISTS idx_sparks_updated_at ON sparks(updated_at);
