use std::collections::HashMap;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{chrono_now, local_today, Database, DbError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoodEntry {
    pub id: String,
    pub name: String,
    pub quantity: f64,
    pub unit: String,
    pub calories: f64,
    pub protein: f64,
    pub carbs: f64,
    pub fat: f64,
    pub date: String,
    pub meal_type: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FridgeItem {
    pub id: String,
    pub name: String,
    pub quantity: f64,
    pub unit: String,
    pub category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub calories: f64,
    pub protein: f64,
    pub carbs: f64,
    pub fat: f64,
    pub ingredients: Vec<String>,
    pub instructions: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightEntry {
    pub id: String,
    pub date: String,
    pub kg: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Climb {
    pub id: String,
    pub name: String,
    pub grade: String,
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub attempts: i64,
    pub sent: bool,
    pub project: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workout {
    pub id: String,
    pub name: String,
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_minutes: Option<i64>,
    pub created_at: i64,
    #[serde(default)]
    pub sets: Vec<WorkoutSet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutSet {
    pub id: String,
    pub workout_id: String,
    pub exercise: String,
    pub set_index: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reps: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rest_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitnessPr {
    pub id: String,
    pub exercise: String,
    pub metric: String,
    pub value: f64,
    pub unit: String,
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workout_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedMeal {
    pub name: String,
    pub calories: f64,
    pub ingredients: Vec<String>,
    pub fridge_matches: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct DetectedPr {
    pub exercise: String,
    pub metric: String,
    pub value: f64,
    pub unit: String,
    pub previous: Option<f64>,
}

pub fn epley_1rm(weight: f64, reps: i64) -> f64 {
    if reps <= 1 {
        weight
    } else {
        weight * (1.0 + reps as f64 / 30.0)
    }
}

pub fn detect_prs(sets: &[WorkoutSet], existing: &[FitnessPr]) -> Vec<DetectedPr> {
    let mut out = Vec::new();
    use std::collections::HashMap;
    let mut by_ex: HashMap<String, Vec<&WorkoutSet>> = HashMap::new();
    for s in sets {
        by_ex.entry(s.exercise.to_lowercase()).or_default().push(s);
    }
    for (exercise, group) in by_ex {
        let best_weight = group.iter().filter_map(|s| s.weight).fold(0.0_f64, f64::max);
        let best_reps = group.iter().filter_map(|s| s.reps).max().unwrap_or(0);
        let best_1rm = group
            .iter()
            .filter_map(|s| match (s.weight, s.reps) {
                (Some(w), Some(r)) if w > 0.0 && r > 0 => Some(epley_1rm(w, r)),
                _ => None,
            })
            .fold(0.0_f64, f64::max);
        let best_dist = group.iter().filter_map(|s| s.distance).fold(0.0_f64, f64::max);
        let best_time = group
            .iter()
            .filter_map(|s| s.duration_seconds)
            .min()
            .map(|t| t as f64);

        let display = group
            .first()
            .map(|s| s.exercise.clone())
            .unwrap_or(exercise.clone());

        push_if_better(&mut out, existing, &display, &exercise, "max_weight", best_weight, "kg");
        if best_reps > 0 {
            push_if_better(
                &mut out,
                existing,
                &display,
                &exercise,
                "max_reps",
                best_reps as f64,
                "reps",
            );
        }
        push_if_better(&mut out, existing, &display, &exercise, "1rm", best_1rm, "kg");
        push_if_better(&mut out, existing, &display, &exercise, "distance", best_dist, "m");
        if let Some(t) = best_time {
            let prev = existing
                .iter()
                .filter(|p| p.exercise.eq_ignore_ascii_case(&exercise) && p.metric == "time")
                .map(|p| p.value)
                .fold(f64::INFINITY, f64::min);
            if prev.is_infinite() || t < prev {
                out.push(DetectedPr {
                    exercise: display.clone(),
                    metric: "time".into(),
                    value: t,
                    unit: "s".into(),
                    previous: if prev.is_infinite() { None } else { Some(prev) },
                });
            }
        }
    }
    out
}

fn push_if_better(
    out: &mut Vec<DetectedPr>,
    existing: &[FitnessPr],
    display: &str,
    exercise_key: &str,
    metric: &str,
    value: f64,
    unit: &str,
) {
    if value <= 0.0 {
        return;
    }
    let prev = existing
        .iter()
        .filter(|p| p.exercise.eq_ignore_ascii_case(exercise_key) && p.metric == metric)
        .map(|p| p.value)
        .fold(0.0_f64, f64::max);
    if value > prev {
        out.push(DetectedPr {
            exercise: display.to_string(),
            metric: metric.to_string(),
            value,
            unit: unit.to_string(),
            previous: if prev > 0.0 { Some(prev) } else { None },
        });
    }
}

pub fn suggest_meals(
    recipes: &[Recipe],
    fridge: &[FridgeItem],
    remaining_calories: f64,
) -> Vec<SuggestedMeal> {
    let fridge_names: Vec<String> = fridge.iter().map(|f| f.name.to_lowercase()).collect();
    let mut scored: Vec<(i32, SuggestedMeal)> = recipes
        .iter()
        .filter(|r| remaining_calories <= 0.0 || r.calories <= remaining_calories + 80.0)
        .map(|r| {
            let matches: Vec<String> = r
                .ingredients
                .iter()
                .filter(|ing| {
                    fridge_names.iter().any(|f| f.contains(&ing.to_lowercase()) || ing.to_lowercase().contains(f))
                })
                .cloned()
                .collect();
            let score = matches.len() as i32 * 10 - (r.calories - remaining_calories.min(r.calories)).abs() as i32 / 20;
            (
                score,
                SuggestedMeal {
                    name: r.name.clone(),
                    calories: r.calories,
                    ingredients: r.ingredients.clone(),
                    fridge_matches: matches.clone(),
                    reason: if remaining_calories > 0.0 {
                        format!(
                            "Fits ~{:.0} remaining kcal; {} fridge match(es).",
                            remaining_calories,
                            matches.len()
                        )
                    } else {
                        "Suggestion only — no remaining calorie budget set.".into()
                    },
                },
            )
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, m)| m).take(5).collect()
}

impl Database {
    fn map_food_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FoodEntry> {
        Ok(FoodEntry {
            id: row.get(0)?,
            name: row.get(1)?,
            quantity: row.get(2)?,
            unit: row.get(3)?,
            calories: row.get(4)?,
            protein: row.get(5)?,
            carbs: row.get(6)?,
            fat: row.get(7)?,
            date: row.get(8)?,
            meal_type: row.get(9)?,
            created_at: row.get(10)?,
        })
    }

    pub fn list_food_entries(&self, date: &str) -> Result<Vec<FoodEntry>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, quantity, unit, calories, protein, carbs, fat, date, meal_type, created_at FROM food_entries WHERE date=?1 ORDER BY created_at",
            )?;
            let rows = stmt.query_map(params![date], Self::map_food_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn list_all_food_entries(&self) -> Result<Vec<FoodEntry>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, quantity, unit, calories, protein, carbs, fat, date, meal_type, created_at FROM food_entries ORDER BY date DESC, created_at DESC LIMIT 2000",
            )?;
            let rows = stmt.query_map([], Self::map_food_row)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn upsert_food_entry(&self, e: FoodEntry) -> Result<FoodEntry, DbError> {
        let now = chrono_now();
        let mut e = e;
        if e.id.is_empty() {
            e.id = Uuid::new_v4().to_string();
        }
        if e.created_at == 0 {
            e.created_at = now;
        }
        if e.date.is_empty() {
            e.date = local_today();
        }
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO food_entries (id, name, quantity, unit, calories, protein, carbs, fat, date, meal_type, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
                 ON CONFLICT(id) DO UPDATE SET name=excluded.name, quantity=excluded.quantity, unit=excluded.unit,
                    calories=excluded.calories, protein=excluded.protein, carbs=excluded.carbs, fat=excluded.fat,
                    date=excluded.date, meal_type=excluded.meal_type",
                params![e.id, e.name, e.quantity, e.unit, e.calories, e.protein, e.carbs, e.fat, e.date, e.meal_type, e.created_at],
            )?;
            Ok(())
        })?;
        Ok(e)
    }

    pub fn delete_food_entry(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM food_entries WHERE id=?1", params![id])?;
            if n == 0 { Err(DbError::NotFound(id.into())) } else { Ok(()) }
        })
    }

    pub fn list_fridge(&self) -> Result<Vec<FridgeItem>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, quantity, unit, category, expiry_date, created_at, updated_at FROM fridge_items ORDER BY name",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(FridgeItem {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    quantity: row.get(2)?,
                    unit: row.get(3)?,
                    category: row.get(4)?,
                    expiry_date: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn upsert_fridge_item(&self, item: FridgeItem) -> Result<FridgeItem, DbError> {
        let now = chrono_now();
        let mut i = item;
        if i.id.is_empty() {
            i.id = Uuid::new_v4().to_string();
            i.created_at = now;
        }
        i.updated_at = now;
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO fridge_items (id, name, quantity, unit, category, expiry_date, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
                 ON CONFLICT(id) DO UPDATE SET name=excluded.name, quantity=excluded.quantity, unit=excluded.unit,
                    category=excluded.category, expiry_date=excluded.expiry_date, updated_at=excluded.updated_at",
                params![i.id, i.name, i.quantity, i.unit, i.category, i.expiry_date, i.created_at, i.updated_at],
            )?;
            Ok(())
        })?;
        Ok(i)
    }

    pub fn delete_fridge_item(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM fridge_items WHERE id=?1", params![id])?;
            if n == 0 { Err(DbError::NotFound(id.into())) } else { Ok(()) }
        })
    }

    pub fn list_recipes(&self) -> Result<Vec<Recipe>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, calories, protein, carbs, fat, ingredients_json, instructions, created_at FROM recipes ORDER BY name",
            )?;
            let rows = stmt.query_map([], |row| {
                let raw: String = row.get(6)?;
                Ok(Recipe {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    calories: row.get(2)?,
                    protein: row.get(3)?,
                    carbs: row.get(4)?,
                    fat: row.get(5)?,
                    ingredients: serde_json::from_str(&raw).unwrap_or_default(),
                    instructions: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn upsert_recipe(&self, r: Recipe) -> Result<Recipe, DbError> {
        let now = chrono_now();
        let mut r = r;
        if r.id.is_empty() {
            r.id = Uuid::new_v4().to_string();
            r.created_at = now;
        }
        let ingredients = serde_json::to_string(&r.ingredients).unwrap_or_else(|_| "[]".into());
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO recipes (id, name, calories, protein, carbs, fat, ingredients_json, instructions, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
                 ON CONFLICT(id) DO UPDATE SET name=excluded.name, calories=excluded.calories, protein=excluded.protein,
                    carbs=excluded.carbs, fat=excluded.fat, ingredients_json=excluded.ingredients_json, instructions=excluded.instructions",
                params![r.id, r.name, r.calories, r.protein, r.carbs, r.fat, ingredients, r.instructions, r.created_at],
            )?;
            Ok(())
        })?;
        Ok(r)
    }

    pub fn list_weight_entries(&self) -> Result<Vec<WeightEntry>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, date, kg, notes, created_at FROM weight_entries ORDER BY date DESC LIMIT 1000",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(WeightEntry {
                    id: row.get(0)?,
                    date: row.get(1)?,
                    kg: row.get(2)?,
                    notes: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn upsert_weight_entry(&self, e: WeightEntry) -> Result<WeightEntry, DbError> {
        let now = chrono_now();
        let mut e = e;
        if e.id.is_empty() {
            e.id = Uuid::new_v4().to_string();
        }
        if e.created_at == 0 {
            e.created_at = now;
        }
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO weight_entries (id, date, kg, notes, created_at) VALUES (?1,?2,?3,?4,?5)
                 ON CONFLICT(id) DO UPDATE SET date=excluded.date, kg=excluded.kg, notes=excluded.notes",
                params![e.id, e.date, e.kg, e.notes, e.created_at],
            )?;
            Ok(())
        })?;
        Ok(e)
    }

    pub fn delete_weight_entry(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM weight_entries WHERE id=?1", params![id])?;
            if n == 0 { Err(DbError::NotFound(id.into())) } else { Ok(()) }
        })
    }

    pub fn list_climbs(&self) -> Result<Vec<Climb>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, grade, date, location, attempts, sent, project, style, notes, created_at FROM climbs ORDER BY date DESC LIMIT 1000",
            )?;
            let rows = stmt.query_map([], |row| {
                let sent: i64 = row.get(6)?;
                let project: i64 = row.get(7)?;
                Ok(Climb {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    grade: row.get(2)?,
                    date: row.get(3)?,
                    location: row.get(4)?,
                    attempts: row.get(5)?,
                    sent: sent != 0,
                    project: project != 0,
                    style: row.get(8)?,
                    notes: row.get(9)?,
                    created_at: row.get(10)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn upsert_climb(&self, c: Climb) -> Result<Climb, DbError> {
        let now = chrono_now();
        let mut c = c;
        if c.id.is_empty() {
            c.id = Uuid::new_v4().to_string();
            c.created_at = now;
        }
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO climbs (id, name, grade, date, location, attempts, sent, project, style, notes, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
                 ON CONFLICT(id) DO UPDATE SET name=excluded.name, grade=excluded.grade, date=excluded.date,
                    location=excluded.location, attempts=excluded.attempts, sent=excluded.sent, project=excluded.project,
                    style=excluded.style, notes=excluded.notes",
                params![c.id, c.name, c.grade, c.date, c.location, c.attempts, if c.sent {1} else {0}, if c.project {1} else {0}, c.style, c.notes, c.created_at],
            )?;
            Ok(())
        })?;
        Ok(c)
    }

    pub fn delete_climb(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            let n = conn.execute("DELETE FROM climbs WHERE id=?1", params![id])?;
            if n == 0 { Err(DbError::NotFound(id.into())) } else { Ok(()) }
        })
    }

    pub fn list_workouts(&self, limit: i64) -> Result<Vec<Workout>, DbError> {
        let mut workouts = self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, date, notes, duration_minutes, created_at FROM workouts ORDER BY date DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok(Workout {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    date: row.get(2)?,
                    notes: row.get(3)?,
                    duration_minutes: row.get(4)?,
                    created_at: row.get(5)?,
                    sets: vec![],
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })?;
        if workouts.is_empty() {
            return Ok(workouts);
        }
        let mut sets_by_workout: HashMap<String, Vec<WorkoutSet>> = HashMap::new();
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, workout_id, exercise, set_index, reps, weight, duration_seconds, rest_seconds, distance, notes
                 FROM workout_sets WHERE workout_id IN (SELECT id FROM workouts ORDER BY date DESC LIMIT ?1)
                 ORDER BY workout_id, set_index",
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok(WorkoutSet {
                    id: row.get(0)?,
                    workout_id: row.get(1)?,
                    exercise: row.get(2)?,
                    set_index: row.get(3)?,
                    reps: row.get(4)?,
                    weight: row.get(5)?,
                    duration_seconds: row.get(6)?,
                    rest_seconds: row.get(7)?,
                    distance: row.get(8)?,
                    notes: row.get(9)?,
                })
            })?;
            for set in rows {
                let set = set?;
                sets_by_workout.entry(set.workout_id.clone()).or_default().push(set);
            }
            Ok(())
        })?;
        for w in &mut workouts {
            w.sets = sets_by_workout.remove(&w.id).unwrap_or_default();
        }
        Ok(workouts)
    }

    pub fn list_workout_sets(&self, workout_id: &str) -> Result<Vec<WorkoutSet>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, workout_id, exercise, set_index, reps, weight, duration_seconds, rest_seconds, distance, notes FROM workout_sets WHERE workout_id=?1 ORDER BY set_index",
            )?;
            let rows = stmt.query_map(params![workout_id], |row| {
                Ok(WorkoutSet {
                    id: row.get(0)?,
                    workout_id: row.get(1)?,
                    exercise: row.get(2)?,
                    set_index: row.get(3)?,
                    reps: row.get(4)?,
                    weight: row.get(5)?,
                    duration_seconds: row.get(6)?,
                    rest_seconds: row.get(7)?,
                    distance: row.get(8)?,
                    notes: row.get(9)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn save_workout(&self, workout: Workout) -> Result<(Workout, Vec<DetectedPr>), DbError> {
        let now = chrono_now();
        let mut w = workout;
        if w.id.is_empty() {
            w.id = Uuid::new_v4().to_string();
            w.created_at = now;
        }
        if w.date.is_empty() {
            w.date = local_today();
        }
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO workouts (id, name, date, notes, duration_minutes, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6)
                 ON CONFLICT(id) DO UPDATE SET name=excluded.name, date=excluded.date, notes=excluded.notes, duration_minutes=excluded.duration_minutes",
                params![w.id, w.name, w.date, w.notes, w.duration_minutes, w.created_at],
            )?;
            conn.execute("DELETE FROM workout_sets WHERE workout_id=?1", params![w.id])?;
            Ok(())
        })?;
        let mut saved_sets = Vec::new();
        for (i, mut set) in w.sets.into_iter().enumerate() {
            set.workout_id = w.id.clone();
            if set.id.is_empty() {
                set.id = Uuid::new_v4().to_string();
            }
            set.set_index = (i as i64) + 1;
            self.with_conn(|conn| {
                conn.execute(
                    "INSERT INTO workout_sets (id, workout_id, exercise, set_index, reps, weight, duration_seconds, rest_seconds, distance, notes)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                    params![set.id, set.workout_id, set.exercise, set.set_index, set.reps, set.weight, set.duration_seconds, set.rest_seconds, set.distance, set.notes],
                )?;
                Ok(())
            })?;
            saved_sets.push(set);
        }
        w.sets = saved_sets;
        let existing = self.list_prs()?;
        let detected = detect_prs(&w.sets, &existing);
        for pr in &detected {
            self.with_conn(|conn| {
                conn.execute(
                    "INSERT INTO fitness_prs (id, exercise, metric, value, unit, date, workout_id, notes, created_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                    params![
                        Uuid::new_v4().to_string(),
                        pr.exercise,
                        pr.metric,
                        pr.value,
                        pr.unit,
                        w.date,
                        w.id,
                        pr.previous.map(|p| format!("previous: {p}")),
                        now,
                    ],
                )?;
                Ok(())
            })?;
        }
        Ok((w, detected))
    }

    pub fn delete_workout(&self, id: &str) -> Result<(), DbError> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM workout_sets WHERE workout_id=?1", params![id])?;
            let n = conn.execute("DELETE FROM workouts WHERE id=?1", params![id])?;
            if n == 0 { Err(DbError::NotFound(id.into())) } else { Ok(()) }
        })
    }

    pub fn list_prs(&self) -> Result<Vec<FitnessPr>, DbError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, exercise, metric, value, unit, date, workout_id, notes, created_at FROM fitness_prs ORDER BY date DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(FitnessPr {
                    id: row.get(0)?,
                    exercise: row.get(1)?,
                    metric: row.get(2)?,
                    value: row.get(3)?,
                    unit: row.get(4)?,
                    date: row.get(5)?,
                    workout_id: row.get(6)?,
                    notes: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
        })
    }

    pub fn workouts_this_week(&self) -> Result<i64, DbError> {
        let today = local_today();
        let start = crate::todos::add_days_to_date(&today, -6).unwrap_or(today.clone());
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM workouts WHERE date >= ?1 AND date <= ?2",
                params![start, today],
                |row| row.get(0),
            )
            .map_err(DbError::from)
        })
    }

    pub fn format_fitness_digest(&self, calorie_target: f64) -> Option<String> {
        let today = local_today();
        let foods = self.list_food_entries(&today).ok()?;
        let eaten: f64 = foods.iter().map(|f| f.calories).sum();
        let remaining = calorie_target - eaten;
        let workouts = self.workouts_this_week().ok().unwrap_or(0);
        let weight = self
            .list_weight_entries()
            .ok()
            .and_then(|w| w.first().map(|e| format!("{:.1}kg", e.kg)))
            .unwrap_or_else(|| "—".into());
        Some(format!(
            "Fitness today: {eaten:.0}/{calorie_target:.0} kcal ({remaining:.0} remaining). Weight {weight}. Workouts this week: {workouts}."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_new_max_weight() {
        let sets = vec![WorkoutSet {
            id: "1".into(),
            workout_id: "w".into(),
            exercise: "Bench Press".into(),
            set_index: 1,
            reps: Some(5),
            weight: Some(100.0),
            duration_seconds: None,
            rest_seconds: None,
            distance: None,
            notes: None,
        }];
        let existing = vec![FitnessPr {
            id: "p".into(),
            exercise: "Bench Press".into(),
            metric: "max_weight".into(),
            value: 95.0,
            unit: "kg".into(),
            date: "2026-01-01".into(),
            workout_id: None,
            notes: None,
            created_at: 0,
        }];
        let prs = detect_prs(&sets, &existing);
        assert!(prs.iter().any(|p| p.metric == "max_weight" && p.value == 100.0));
    }

    #[test]
    fn meals_prefer_fridge_and_remaining() {
        let recipes = vec![
            Recipe {
                id: "a".into(),
                name: "Chicken rice".into(),
                calories: 600.0,
                protein: 40.0,
                carbs: 50.0,
                fat: 10.0,
                ingredients: vec!["chicken".into(), "rice".into()],
                instructions: String::new(),
                created_at: 0,
            },
            Recipe {
                id: "b".into(),
                name: "Huge feast".into(),
                calories: 2000.0,
                protein: 80.0,
                carbs: 200.0,
                fat: 80.0,
                ingredients: vec!["everything".into()],
                instructions: String::new(),
                created_at: 0,
            },
        ];
        let fridge = vec![FridgeItem {
            id: "f".into(),
            name: "Chicken".into(),
            quantity: 1.0,
            unit: "kg".into(),
            category: "meat".into(),
            expiry_date: None,
            created_at: 0,
            updated_at: 0,
        }];
        let meals = suggest_meals(&recipes, &fridge, 800.0);
        assert_eq!(meals[0].name, "Chicken rice");
        assert!(!meals.iter().any(|m| m.name == "Huge feast"));
    }
}
