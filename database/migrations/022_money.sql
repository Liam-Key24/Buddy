CREATE TABLE IF NOT EXISTS money_entries (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    date TEXT NOT NULL,
    description TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT 'general',
    amount_cents INTEGER NOT NULL,
    year INTEGER NOT NULL,
    month INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_money_year_month ON money_entries(year, month);
CREATE INDEX IF NOT EXISTS idx_money_kind ON money_entries(kind);
CREATE INDEX IF NOT EXISTS idx_money_category ON money_entries(category);
