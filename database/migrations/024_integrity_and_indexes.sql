-- Integrity, FKs, CHECKs, and covering indexes.
-- Null dangling optional FKs before rebuilding tables that gain REFERENCES.

UPDATE social_ideas SET thread_id = NULL
 WHERE thread_id IS NOT NULL
   AND thread_id NOT IN (SELECT id FROM social_threads);

UPDATE social_drafts SET thread_id = NULL
 WHERE thread_id IS NOT NULL
   AND thread_id NOT IN (SELECT id FROM social_threads);

UPDATE social_posts SET thread_id = NULL
 WHERE thread_id IS NOT NULL
   AND thread_id NOT IN (SELECT id FROM social_threads);

UPDATE social_drafts SET source_post_id = NULL
 WHERE source_post_id IS NOT NULL
   AND source_post_id NOT IN (SELECT id FROM social_posts);

UPDATE fitness_prs SET workout_id = NULL
 WHERE workout_id IS NOT NULL
   AND workout_id NOT IN (SELECT id FROM workouts);

UPDATE todos SET status = 'not_started'
 WHERE status NOT IN ('not_started', 'in_progress', 'completed');
UPDATE todos SET priority = 'medium'
 WHERE priority NOT IN ('low', 'medium', 'high', 'critical');
UPDATE todos SET recurrence = 'none'
 WHERE recurrence NOT IN ('none', 'daily', 'weekly', 'monthly');

UPDATE money_entries SET kind = 'expense'
 WHERE kind NOT IN ('income', 'expense');

UPDATE social_posts SET status = 'proposed'
 WHERE status NOT IN ('proposed', 'approved', 'rejected', 'published', 'drafting');

PRAGMA foreign_keys = OFF;

CREATE TABLE social_ideas_new (
    id TEXT PRIMARY KEY,
    body TEXT NOT NULL,
    platform TEXT,
    thread_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    used_at INTEGER,
    FOREIGN KEY (thread_id) REFERENCES social_threads(id) ON DELETE SET NULL
);
INSERT INTO social_ideas_new (id, body, platform, thread_id, created_at, updated_at, used_at)
    SELECT id, body, platform, thread_id, created_at, updated_at, used_at FROM social_ideas;
DROP TABLE social_ideas;
ALTER TABLE social_ideas_new RENAME TO social_ideas;

CREATE TABLE social_posts_new (
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
    status TEXT NOT NULL DEFAULT 'proposed'
        CHECK (status IN ('proposed', 'approved', 'rejected', 'published', 'drafting')),
    calendar_event_id TEXT,
    metrics_json TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (plan_id) REFERENCES social_weekly_plans(id) ON DELETE CASCADE,
    FOREIGN KEY (thread_id) REFERENCES social_threads(id) ON DELETE SET NULL
);
INSERT INTO social_posts_new SELECT * FROM social_posts;
DROP TABLE social_posts;
ALTER TABLE social_posts_new RENAME TO social_posts;

CREATE TABLE social_drafts_new (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL,
    platform TEXT NOT NULL DEFAULT 'x',
    thread_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0,
    source_post_id TEXT,
    FOREIGN KEY (thread_id) REFERENCES social_threads(id) ON DELETE SET NULL,
    FOREIGN KEY (source_post_id) REFERENCES social_posts(id) ON DELETE SET NULL
);
INSERT INTO social_drafts_new (id, title, body, platform, thread_id, created_at, updated_at, archived, source_post_id)
    SELECT id, title, body, platform, thread_id, created_at, updated_at, archived, source_post_id FROM social_drafts;
DROP TABLE social_drafts;
ALTER TABLE social_drafts_new RENAME TO social_drafts;

CREATE TABLE fitness_prs_new (
    id TEXT PRIMARY KEY,
    exercise TEXT NOT NULL,
    metric TEXT NOT NULL,
    value REAL NOT NULL,
    unit TEXT NOT NULL DEFAULT '',
    date TEXT NOT NULL,
    workout_id TEXT,
    notes TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (workout_id) REFERENCES workouts(id) ON DELETE SET NULL
);
INSERT INTO fitness_prs_new SELECT * FROM fitness_prs;
DROP TABLE fitness_prs;
ALTER TABLE fitness_prs_new RENAME TO fitness_prs;

CREATE TABLE todos_new (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    deadline TEXT,
    priority TEXT NOT NULL DEFAULT 'medium'
        CHECK (priority IN ('low', 'medium', 'high', 'critical')),
    status TEXT NOT NULL DEFAULT 'not_started'
        CHECK (status IN ('not_started', 'in_progress', 'completed')),
    category TEXT NOT NULL DEFAULT 'general',
    notes TEXT,
    recurrence TEXT NOT NULL DEFAULT 'none'
        CHECK (recurrence IN ('none', 'daily', 'weekly', 'monthly')),
    completed_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
INSERT INTO todos_new SELECT * FROM todos;
DROP TABLE todos;
ALTER TABLE todos_new RENAME TO todos;

CREATE TABLE money_entries_new (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('income', 'expense')),
    date TEXT NOT NULL,
    description TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT 'general',
    amount_cents INTEGER NOT NULL,
    year INTEGER NOT NULL,
    month INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
INSERT INTO money_entries_new SELECT * FROM money_entries;
DROP TABLE money_entries;
ALTER TABLE money_entries_new RENAME TO money_entries;

PRAGMA foreign_keys = ON;

CREATE INDEX IF NOT EXISTS idx_todos_status ON todos(status);
CREATE INDEX IF NOT EXISTS idx_todos_deadline ON todos(deadline);
CREATE INDEX IF NOT EXISTS idx_todos_priority ON todos(priority);
CREATE INDEX IF NOT EXISTS idx_todos_category ON todos(category);
CREATE INDEX IF NOT EXISTS idx_todos_status_deadline ON todos(status, deadline);

CREATE INDEX IF NOT EXISTS idx_money_year_month ON money_entries(year, month);
CREATE INDEX IF NOT EXISTS idx_money_kind ON money_entries(kind);
CREATE INDEX IF NOT EXISTS idx_money_category ON money_entries(category);

CREATE INDEX IF NOT EXISTS idx_social_posts_plan ON social_posts(plan_id);
CREATE INDEX IF NOT EXISTS idx_social_posts_status ON social_posts(status);
CREATE INDEX IF NOT EXISTS idx_social_posts_date ON social_posts(slot_date);
CREATE INDEX IF NOT EXISTS idx_social_ideas_thread ON social_ideas(thread_id);
CREATE INDEX IF NOT EXISTS idx_social_drafts_archived_updated ON social_drafts(archived, updated_at);
CREATE INDEX IF NOT EXISTS idx_social_drafts_source_post ON social_drafts(source_post_id);

CREATE INDEX IF NOT EXISTS idx_prs_exercise ON fitness_prs(exercise);
CREATE INDEX IF NOT EXISTS idx_prs_workout ON fitness_prs(workout_id);

CREATE INDEX IF NOT EXISTS idx_study_assignments_subject ON study_assignments(subject_id);
CREATE INDEX IF NOT EXISTS idx_study_sessions_subject ON study_sessions(subject_id);
CREATE INDEX IF NOT EXISTS idx_study_sessions_topic ON study_sessions(topic_id);
