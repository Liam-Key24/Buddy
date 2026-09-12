-- Goal-to-calendar records. Separate from workspace_profiles.goals (code-workspace notes).

CREATE TABLE IF NOT EXISTS buddy_goals (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    desired_outcome TEXT,
    motivation TEXT,
    start_date TEXT,
    deadline TEXT,
    priority TEXT NOT NULL DEFAULT 'medium',
    status TEXT NOT NULL DEFAULT 'active',
    progress_method TEXT,
    current_forecast TEXT NOT NULL DEFAULT 'on_track',
    protected_constraints TEXT,
    source_spark_id TEXT,
    assumptions_json TEXT,
    current_value REAL,
    target_value REAL,
    unit TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS buddy_milestones (
    id TEXT PRIMARY KEY,
    goal_id TEXT NOT NULL,
    outcome TEXT NOT NULL,
    target_date TEXT,
    ordering INTEGER NOT NULL DEFAULT 0,
    completion_rule TEXT,
    dependencies_json TEXT,
    status TEXT NOT NULL DEFAULT 'open',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(goal_id) REFERENCES buddy_goals(id)
);

CREATE TABLE IF NOT EXISTS buddy_planned_actions (
    id TEXT PRIMARY KEY,
    goal_id TEXT NOT NULL,
    milestone_id TEXT,
    description TEXT NOT NULL,
    estimated_minutes INTEGER,
    energy TEXT,
    frequency TEXT,
    earliest_start TEXT,
    deadline TEXT,
    flexibility TEXT NOT NULL DEFAULT 'flexible',
    priority TEXT NOT NULL DEFAULT 'medium',
    calendar_event_id TEXT,
    status TEXT NOT NULL DEFAULT 'planned',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY(goal_id) REFERENCES buddy_goals(id)
);

CREATE TABLE IF NOT EXISTS buddy_progress_signals (
    id TEXT PRIMARY KEY,
    goal_id TEXT NOT NULL,
    source TEXT NOT NULL,
    measurement REAL,
    unit TEXT,
    recorded_at INTEGER NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0,
    notes TEXT,
    confirmed INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(goal_id) REFERENCES buddy_goals(id)
);

CREATE TABLE IF NOT EXISTS buddy_reviews (
    id TEXT PRIMARY KEY,
    goal_id TEXT,
    expected_position TEXT,
    actual_position TEXT,
    forecast TEXT,
    blockers_json TEXT,
    estimation_errors TEXT,
    recommended_adaptations_json TEXT,
    user_decision TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS buddy_goal_history (
    id TEXT PRIMARY KEY,
    goal_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    payload TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_buddy_goals_status ON buddy_goals(status);
CREATE INDEX IF NOT EXISTS idx_buddy_goals_deadline ON buddy_goals(deadline);
CREATE INDEX IF NOT EXISTS idx_buddy_planned_actions_goal ON buddy_planned_actions(goal_id);
CREATE INDEX IF NOT EXISTS idx_buddy_progress_goal ON buddy_progress_signals(goal_id);
CREATE INDEX IF NOT EXISTS idx_buddy_history_goal ON buddy_goal_history(goal_id);
