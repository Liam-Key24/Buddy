CREATE TABLE IF NOT EXISTS social_profile (
    id TEXT PRIMARY KEY CHECK (id = 'default'),
    narrative TEXT NOT NULL,
    tone_notes TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL
);

INSERT OR IGNORE INTO social_profile (id, narrative, tone_notes, updated_at)
VALUES (
    'default',
    'A solo developer building apps for profit while teaching himself application security and cloud security.',
    'Intelligent, curious, honest, technical when appropriate, personal, practical, occasionally humorous. Not corporate or motivational-guru. The journey is the content.',
    0
);

CREATE TABLE IF NOT EXISTS social_threads (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    current_chapter TEXT NOT NULL DEFAULT '',
    sort_order INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL
);

INSERT OR IGNORE INTO social_threads (id, name, current_chapter, sort_order, updated_at) VALUES
    ('building_apps', 'Building Apps', '', 0, 0),
    ('making_money', 'Making Money', '', 1, 0),
    ('app_security', 'Application Security', '', 2, 0),
    ('cloud_security', 'Cloud Security', '', 3, 0),
    ('self_study', 'Self-Study', '', 4, 0);

CREATE TABLE IF NOT EXISTS social_projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    notes TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS social_ideas (
    id TEXT PRIMARY KEY,
    body TEXT NOT NULL,
    platform TEXT,
    thread_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS social_drafts (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL,
    platform TEXT NOT NULL DEFAULT 'x',
    thread_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS social_weekly_plans (
    id TEXT PRIMARY KEY,
    week_start TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'drafting',
    last_week_notes TEXT NOT NULL DEFAULT '',
    context_digest TEXT NOT NULL DEFAULT '',
    results_json TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS social_posts (
    id TEXT PRIMARY KEY,
    plan_id TEXT NOT NULL,
    platform TEXT NOT NULL,
    slot_date TEXT NOT NULL,
    slot_time TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT '',
    thread_id TEXT,
    purpose TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT '',
    suggested_media TEXT NOT NULL DEFAULT '',
    gather_json TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'proposed',
    calendar_event_id TEXT,
    metrics_json TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (plan_id) REFERENCES social_weekly_plans(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_social_posts_plan ON social_posts(plan_id);
CREATE INDEX IF NOT EXISTS idx_social_posts_status ON social_posts(status);
CREATE INDEX IF NOT EXISTS idx_social_posts_date ON social_posts(slot_date);
CREATE INDEX IF NOT EXISTS idx_social_projects_status ON social_projects(status);
