CREATE TABLE IF NOT EXISTS food_entries (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    quantity REAL NOT NULL DEFAULT 1,
    unit TEXT NOT NULL DEFAULT 'serving',
    calories REAL NOT NULL DEFAULT 0,
    protein REAL NOT NULL DEFAULT 0,
    carbs REAL NOT NULL DEFAULT 0,
    fat REAL NOT NULL DEFAULT 0,
    date TEXT NOT NULL,
    meal_type TEXT NOT NULL DEFAULT 'snack',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS fridge_items (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    quantity REAL NOT NULL DEFAULT 1,
    unit TEXT NOT NULL DEFAULT 'item',
    category TEXT NOT NULL DEFAULT 'other',
    expiry_date TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS recipes (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    calories REAL NOT NULL DEFAULT 0,
    protein REAL NOT NULL DEFAULT 0,
    carbs REAL NOT NULL DEFAULT 0,
    fat REAL NOT NULL DEFAULT 0,
    ingredients_json TEXT NOT NULL DEFAULT '[]',
    instructions TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS weight_entries (
    id TEXT PRIMARY KEY,
    date TEXT NOT NULL,
    kg REAL NOT NULL,
    notes TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS climbs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    grade TEXT NOT NULL,
    date TEXT NOT NULL,
    location TEXT,
    attempts INTEGER NOT NULL DEFAULT 1,
    sent INTEGER NOT NULL DEFAULT 0,
    project INTEGER NOT NULL DEFAULT 0,
    style TEXT,
    notes TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS workouts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    date TEXT NOT NULL,
    notes TEXT,
    duration_minutes INTEGER,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS workout_sets (
    id TEXT PRIMARY KEY,
    workout_id TEXT NOT NULL,
    exercise TEXT NOT NULL,
    set_index INTEGER NOT NULL DEFAULT 1,
    reps INTEGER,
    weight REAL,
    duration_seconds INTEGER,
    rest_seconds INTEGER,
    distance REAL,
    notes TEXT,
    FOREIGN KEY (workout_id) REFERENCES workouts(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS fitness_prs (
    id TEXT PRIMARY KEY,
    exercise TEXT NOT NULL,
    metric TEXT NOT NULL,
    value REAL NOT NULL,
    unit TEXT NOT NULL DEFAULT '',
    date TEXT NOT NULL,
    workout_id TEXT,
    notes TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_food_date ON food_entries(date);
CREATE INDEX IF NOT EXISTS idx_fridge_expiry ON fridge_items(expiry_date);
CREATE INDEX IF NOT EXISTS idx_weight_date ON weight_entries(date);
CREATE INDEX IF NOT EXISTS idx_climbs_date ON climbs(date);
CREATE INDEX IF NOT EXISTS idx_climbs_grade ON climbs(grade);
CREATE INDEX IF NOT EXISTS idx_workouts_date ON workouts(date);
CREATE INDEX IF NOT EXISTS idx_workout_sets_workout ON workout_sets(workout_id);
CREATE INDEX IF NOT EXISTS idx_prs_exercise ON fitness_prs(exercise);

INSERT OR IGNORE INTO recipes (id, name, calories, protein, carbs, fat, ingredients_json, instructions, created_at) VALUES
    ('seed-chicken-rice', 'Chicken, rice & veg', 620, 48, 62, 16, '["chicken","rice","vegetables"]', 'Grill chicken, steam rice and veg.', 0),
    ('seed-egg-toast', 'Eggs on toast', 380, 22, 32, 16, '["eggs","bread","butter"]', 'Fry eggs, toast bread.', 0),
    ('seed-oats', 'Overnight oats', 350, 14, 52, 8, '["oats","milk","banana"]', 'Mix oats with milk and banana, chill.', 0),
    ('seed-tuna-wrap', 'Tuna wrap', 420, 32, 38, 14, '["tuna","wrap","salad"]', 'Mix tuna, wrap with salad.', 0),
    ('seed-stir-fry', 'Quick stir fry', 540, 28, 48, 22, '["rice","eggs","vegetables","soy sauce"]', 'Stir-fry veg and eggs over rice.', 0);
