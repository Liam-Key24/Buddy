-- Agent support: distinguish Buddy chat from Codex code-agent conversations,
-- and add audit trails for filesystem and external actions.
ALTER TABLE conversations ADD COLUMN kind TEXT NOT NULL DEFAULT 'buddy';
ALTER TABLE conversations ADD COLUMN focus_mode TEXT;
ALTER TABLE conversations ADD COLUMN workspace_path TEXT;

CREATE INDEX IF NOT EXISTS idx_conversations_kind ON conversations(kind);

CREATE TABLE IF NOT EXISTS external_actions (
    id TEXT PRIMARY KEY,
    action_type TEXT NOT NULL,
    summary TEXT NOT NULL,
    detail_json TEXT,
    approved INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_external_actions_created_at
  ON external_actions(created_at DESC);

CREATE TABLE IF NOT EXISTS file_operations (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL,
    op TEXT NOT NULL,
    conversation_id TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_file_operations_created_at
  ON file_operations(created_at DESC);
