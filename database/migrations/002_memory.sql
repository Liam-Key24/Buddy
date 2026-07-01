CREATE TABLE IF NOT EXISTS memory_working (
    id TEXT PRIMARY KEY,
    workspace_path TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    embedding BLOB,
    importance REAL
);

CREATE TABLE IF NOT EXISTS memory_project (
    id TEXT PRIMARY KEY,
    workspace_path TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    embedding BLOB,
    importance REAL
);

CREATE TABLE IF NOT EXISTS memory_preference (
    id TEXT PRIMARY KEY,
    workspace_path TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    embedding BLOB,
    importance REAL
);

CREATE TABLE IF NOT EXISTS memory_handover (
    id TEXT PRIMARY KEY,
    workspace_path TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    embedding BLOB,
    importance REAL
);

CREATE TABLE IF NOT EXISTS memory_decision (
    id TEXT PRIMARY KEY,
    workspace_path TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    embedding BLOB,
    importance REAL
);

CREATE TABLE IF NOT EXISTS memory_error (
    id TEXT PRIMARY KEY,
    workspace_path TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    embedding BLOB,
    importance REAL
);

CREATE TABLE IF NOT EXISTS memory_tool (
    id TEXT PRIMARY KEY,
    workspace_path TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    embedding BLOB,
    importance REAL
);

CREATE TABLE IF NOT EXISTS memory_reflection (
    id TEXT PRIMARY KEY,
    workspace_path TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    embedding BLOB,
    importance REAL
);

CREATE INDEX IF NOT EXISTS idx_memory_working_workspace ON memory_working(workspace_path);
CREATE INDEX IF NOT EXISTS idx_memory_project_workspace ON memory_project(workspace_path);
CREATE INDEX IF NOT EXISTS idx_memory_preference_workspace ON memory_preference(workspace_path);
CREATE INDEX IF NOT EXISTS idx_memory_handover_workspace ON memory_handover(workspace_path);
CREATE INDEX IF NOT EXISTS idx_memory_decision_workspace ON memory_decision(workspace_path);
CREATE INDEX IF NOT EXISTS idx_memory_error_workspace ON memory_error(workspace_path);
CREATE INDEX IF NOT EXISTS idx_memory_tool_workspace ON memory_tool(workspace_path);
CREATE INDEX IF NOT EXISTS idx_memory_reflection_workspace ON memory_reflection(workspace_path);
