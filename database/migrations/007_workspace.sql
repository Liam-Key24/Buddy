CREATE TABLE IF NOT EXISTS workspace_profiles (
    workspace_path TEXT PRIMARY KEY,
    name TEXT,
    goals TEXT,
    current_milestone TEXT,
    stack_json TEXT,
    architecture_json TEXT,
    features_json TEXT,
    active_tasks_json TEXT,
    recent_decisions_json TEXT,
    known_issues_json TEXT,
    updated_at INTEGER NOT NULL
);
