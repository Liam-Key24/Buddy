CREATE TABLE IF NOT EXISTS learned_patterns (
    id TEXT PRIMARY KEY,
    workspace_path TEXT NOT NULL,
    pattern_type TEXT NOT NULL,
    description TEXT NOT NULL,
    evidence_json TEXT,
    confidence REAL NOT NULL,
    observation_count INTEGER NOT NULL DEFAULT 1,
    last_confirmed_at INTEGER,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_learned_patterns_workspace ON learned_patterns(workspace_path);
CREATE INDEX IF NOT EXISTS idx_learned_patterns_type ON learned_patterns(workspace_path, pattern_type);
