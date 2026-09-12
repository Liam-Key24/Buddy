CREATE TABLE IF NOT EXISTS study_subjects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    color TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS study_topics (
    id TEXT PRIMARY KEY,
    subject_id TEXT NOT NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'not_started',
    last_studied TEXT,
    deadline TEXT,
    priority TEXT NOT NULL DEFAULT 'medium',
    remaining_estimate REAL,
    notes TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (subject_id) REFERENCES study_subjects(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS study_assignments (
    id TEXT PRIMARY KEY,
    subject_id TEXT NOT NULL,
    topic_id TEXT,
    title TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'assignment',
    status TEXT NOT NULL DEFAULT 'not_started',
    deadline TEXT,
    priority TEXT NOT NULL DEFAULT 'medium',
    notes TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (subject_id) REFERENCES study_subjects(id) ON DELETE CASCADE,
    FOREIGN KEY (topic_id) REFERENCES study_topics(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS study_sessions (
    id TEXT PRIMARY KEY,
    subject_id TEXT,
    topic_id TEXT,
    date TEXT NOT NULL,
    duration_minutes INTEGER NOT NULL DEFAULT 0,
    notes TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (subject_id) REFERENCES study_subjects(id) ON DELETE SET NULL,
    FOREIGN KEY (topic_id) REFERENCES study_topics(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_study_topics_subject ON study_topics(subject_id);
CREATE INDEX IF NOT EXISTS idx_study_topics_deadline ON study_topics(deadline);
CREATE INDEX IF NOT EXISTS idx_study_assignments_deadline ON study_assignments(deadline);
CREATE INDEX IF NOT EXISTS idx_study_sessions_date ON study_sessions(date);
