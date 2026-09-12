use std::sync::Arc;

use buddy_core::{
    parse_tool_json, AfterExecute, AskKind, BuddyPlugin, FieldSpec, SettingSeed, Tool, ToolDecl,
    ToolError, ToolResult, ToolSchema,
};
use buddy_database::{
    local_today, suggest_meals, Climb, Database, FoodEntry, FridgeItem, WeightEntry, Workout,
    WorkoutSet,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub struct FitnessPlugin;

struct SummaryTool {
    db: Arc<Database>,
}
struct LookTool {
    db: Arc<Database>,
}
struct LogFoodTool {
    db: Arc<Database>,
}
struct FridgeTool {
    db: Arc<Database>,
}
struct SuggestTool {
    db: Arc<Database>,
}
struct LogWorkoutTool {
    db: Arc<Database>,
}
struct ClimbingTool {
    db: Arc<Database>,
}
struct LogWeightTool {
    db: Arc<Database>,
}

impl BuddyPlugin for FitnessPlugin {
    fn id(&self) -> &'static str {
        "fitness"
    }

    fn tools(&self, db: Arc<Database>) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(SummaryTool { db: db.clone() }),
            Arc::new(LookTool { db: db.clone() }),
            Arc::new(LogFoodTool { db: db.clone() }),
            Arc::new(FridgeTool { db: db.clone() }),
            Arc::new(SuggestTool { db: db.clone() }),
            Arc::new(LogWorkoutTool { db: db.clone() }),
            Arc::new(ClimbingTool { db: db.clone() }),
            Arc::new(LogWeightTool { db }),
        ]
    }

    fn tool_decls(&self) -> &'static [ToolDecl] {
        &[
            ToolDecl {
                name: "fitness.summary",
                planner_line: "fitness.summary: today's food log + macros, weight, recent workouts, climbing snapshot. tool_input JSON: {}",
            },
            ToolDecl {
                name: "fitness.look",
                planner_line: "fitness.look: read tracker rows. tool_input JSON: {\"what\":\"food|workouts|weight|climbs|prs|fridge|recipes|summary\", \"date?\":\"YYYY-MM-DD\", \"limit?\":20}. Use this when they ask what they ate, lifted, weigh, or climbed.",
            },
            ToolDecl {
                name: "fitness.log_food",
                planner_line: "fitness.log_food: log a meal. If the user does not give macros, estimate a typical UK home/restaurant serving and say they are estimates. tool_input JSON: {\"name\":\"chicken rice\", \"calories\":650, \"protein\":40, \"carbs\":70, \"fat\":18, \"quantity\":1, \"unit\":\"serving\", \"meal_type\":\"breakfast|lunch|dinner|snack\", \"date?\":\"YYYY-MM-DD\"}. calories must be > 0.",
            },
            ToolDecl {
                name: "fitness.fridge",
                planner_line: "fitness.fridge: list or add fridge items. tool_input JSON: {\"action\":\"list|add|delete\", \"name?\":\"\", \"quantity?\":1, \"unit?\":\"\", \"category?\":\"\", \"expiry_date?\":\"YYYY-MM-DD\", \"id?\":\"\"}",
            },
            ToolDecl {
                name: "fitness.suggest_meals",
                planner_line: "fitness.suggest_meals: meal ideas from remaining calories + fridge. tool_input JSON: {}",
            },
            ToolDecl {
                name: "fitness.log_workout",
                planner_line: "fitness.log_workout: save a workout and detect PRs. tool_input JSON: {\"name\":\"...\", \"date?\":\"YYYY-MM-DD\", \"sets\":[{\"exercise\":\"Bench Press\",\"reps\":8,\"weight\":70}]}",
            },
            ToolDecl {
                name: "fitness.climbing_stats",
                planner_line: "fitness.climbing_stats: climbing progress V0–V17. tool_input JSON: {\"action?\":\"stats|list|log\", \"name?\":\"\", \"grade?\":\"V4\", \"date?\":\"\", \"sent?\":true, \"attempts?\":3, \"project?\":false, \"location?\":\"\", \"notes?\":\"\"}",
            },
            ToolDecl {
                name: "fitness.log_weight",
                planner_line: "fitness.log_weight: log a weigh-in. tool_input JSON: {\"kg\":82.4, \"date?\":\"YYYY-MM-DD\", \"notes?\":\"\"}",
            },
        ]
    }

    fn tool_schemas(&self) -> &'static [ToolSchema] {
        &[
            ToolSchema {
                tool: "fitness.log_food",
                fields: &[FieldSpec {
                    name: "name",
                    label: "food",
                    required: true,
                    memory_keys: &[],
                    ask_kind: AskKind::Text,
                    choices: &[],
                }],
            },
            ToolSchema {
                tool: "fitness.look",
                fields: &[FieldSpec {
                    name: "what",
                    label: "food, workouts, weight, climbs, prs, fridge, recipes, or summary",
                    required: false,
                    memory_keys: &[],
                    ask_kind: AskKind::Text,
                    choices: &[],
                }],
            },
        ]
    }

    fn setting_seeds(&self) -> &'static [SettingSeed] {
        &[SettingSeed {
            key: "fitness_calorie_target",
            value: "2500",
        }]
    }

    fn after_execute_hint(&self, tool_name: &str) -> AfterExecute {
        match tool_name {
            "fitness.log_food"
            | "fitness.fridge"
            | "fitness.log_workout"
            | "fitness.climbing_stats"
            | "fitness.log_weight" => AfterExecute::EmitFitnessUpdated,
            _ => AfterExecute::None,
        }
    }
}

fn target(db: &Database) -> f64 {
    db.get_setting_or("fitness_calorie_target", "2500")
        .parse()
        .unwrap_or(2500.0)
}

fn climb_grade_rank(g: &str) -> i32 {
    g.trim()
        .trim_start_matches(['v', 'V'])
        .parse::<i32>()
        .unwrap_or(-1)
}

fn highest_grade<'a>(grades: impl Iterator<Item = &'a str>) -> &'a str {
    grades.max_by_key(|g| climb_grade_rank(g)).unwrap_or("—")
}

fn pretty(v: &Value) -> ToolResult {
    ToolResult {
        output: serde_json::to_string_pretty(v).unwrap_or_default(),
    }
}

fn summary_json(db: &Database) -> Result<Value, ToolError> {
    let today = local_today();
    let foods = db
        .list_food_entries(&today)
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
    let eaten: f64 = foods.iter().map(|f| f.calories).sum();
    let protein: f64 = foods.iter().map(|f| f.protein).sum();
    let carbs: f64 = foods.iter().map(|f| f.carbs).sum();
    let fat: f64 = foods.iter().map(|f| f.fat).sum();
    let cal_t = target(db);
    let week_count = db.workouts_this_week().unwrap_or(0);
    let recent = db.list_workouts(8).unwrap_or_default();
    let weights = db.list_weight_entries().unwrap_or_default();
    let climbs = db.list_climbs().unwrap_or_default();
    Ok(json!({
        "date": today,
        "calories_eaten": eaten,
        "calorie_target": cal_t,
        "remaining": cal_t - eaten,
        "protein": protein,
        "carbs": carbs,
        "fat": fat,
        "foods": foods,
        "workouts_this_week": week_count,
        "recent_workouts": recent.iter().map(|w| json!({
            "id": w.id,
            "name": w.name,
            "date": w.date,
            "set_count": w.sets.len(),
        })).collect::<Vec<_>>(),
        "current_weight_kg": weights.first().map(|w| w.kg),
        "starting_weight_kg": weights.last().map(|w| w.kg),
        "highest_climb_grade": highest_grade(climbs.iter().map(|c| c.grade.as_str())),
        "climb_count": climbs.len(),
    }))
}

fn look_json(db: &Database, what: &str, date: Option<&str>, limit: i64) -> Result<Value, ToolError> {
    let today = local_today();
    match what {
        "food" => {
            let day = date.filter(|d| !d.is_empty()).unwrap_or(today.as_str());
            let foods = db
                .list_food_entries(day)
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            Ok(json!({ "date": day, "foods": foods }))
        }
        "workouts" => {
            let workouts = db
                .list_workouts(limit)
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            Ok(json!({ "workouts": workouts }))
        }
        "weight" => {
            let entries = db
                .list_weight_entries()
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            Ok(json!({ "weight": entries }))
        }
        "climbs" => {
            let climbs = db
                .list_climbs()
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            Ok(json!({ "climbs": climbs }))
        }
        "prs" => {
            let prs = db
                .list_prs()
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            Ok(json!({ "prs": prs }))
        }
        "fridge" => {
            let items = db
                .list_fridge()
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            Ok(json!({ "fridge": items }))
        }
        "recipes" => {
            let recipes = db
                .list_recipes()
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            Ok(json!({ "recipes": recipes }))
        }
        _ => summary_json(db),
    }
}

impl Tool for SummaryTool {
    fn name(&self) -> &str {
        "fitness.summary"
    }
    fn execute(&self, _input: &str) -> Result<ToolResult, ToolError> {
        Ok(pretty(&summary_json(&self.db)?))
    }
}

#[derive(Deserialize, Default)]
struct LookIn {
    #[serde(default)]
    what: Option<String>,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    limit: Option<i64>,
}

impl Tool for LookTool {
    fn name(&self) -> &str {
        "fitness.look"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let p: LookIn = parse_tool_json(input, "fitness.look").unwrap_or_default();
        let what = p
            .what
            .as_deref()
            .unwrap_or("summary")
            .trim()
            .to_ascii_lowercase();
        let limit = p.limit.unwrap_or(20).clamp(1, 80);
        Ok(pretty(&look_json(
            &self.db,
            &what,
            p.date.as_deref(),
            limit,
        )?))
    }
}

#[derive(Deserialize)]
struct FoodIn {
    name: String,
    #[serde(default)]
    calories: f64,
    #[serde(default)]
    protein: f64,
    #[serde(default)]
    carbs: f64,
    #[serde(default)]
    fat: f64,
    #[serde(default)]
    quantity: Option<f64>,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    meal_type: Option<String>,
    #[serde(default)]
    date: Option<String>,
}

impl Tool for LogFoodTool {
    fn name(&self) -> &str {
        "fitness.log_food"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let p: FoodIn = parse_tool_json(input, "fitness.log_food")?;
        if p.calories <= 0.0 {
            return Err(ToolError::ExecutionFailed(
                "calories must be > 0. Estimate a typical serving (kcal + protein/carbs/fat grams) and retry. Tell the user they are estimates.".into(),
            ));
        }
        let e = self
            .db
            .upsert_food_entry(FoodEntry {
                id: String::new(),
                name: p.name,
                quantity: p.quantity.unwrap_or(1.0),
                unit: p.unit.unwrap_or_else(|| "serving".into()),
                calories: p.calories,
                protein: p.protein,
                carbs: p.carbs,
                fat: p.fat,
                date: p.date.unwrap_or_default(),
                meal_type: p.meal_type.unwrap_or_else(|| "snack".into()),
                created_at: 0,
            })
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&e).unwrap_or_default(),
        })
    }
}

#[derive(Deserialize)]
struct FridgeIn {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    quantity: Option<f64>,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    expiry_date: Option<String>,
}

impl Tool for FridgeTool {
    fn name(&self) -> &str {
        "fitness.fridge"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let p: FridgeIn = parse_tool_json(input, "fitness.fridge").unwrap_or(FridgeIn {
            action: Some("list".into()),
            id: None,
            name: None,
            quantity: None,
            unit: None,
            category: None,
            expiry_date: None,
        });
        match p.action.as_deref().unwrap_or("list") {
            "delete" => {
                let id = p
                    .id
                    .ok_or_else(|| ToolError::ExecutionFailed("id required".into()))?;
                self.db
                    .delete_fridge_item(&id)
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok(ToolResult {
                    output: format!("Deleted fridge item {id}"),
                })
            }
            "add" => {
                let item = self
                    .db
                    .upsert_fridge_item(FridgeItem {
                        id: String::new(),
                        name: p.name.unwrap_or_default(),
                        quantity: p.quantity.unwrap_or(1.0),
                        unit: p.unit.unwrap_or_else(|| "item".into()),
                        category: p.category.unwrap_or_else(|| "other".into()),
                        expiry_date: p.expiry_date,
                        created_at: 0,
                        updated_at: 0,
                    })
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok(ToolResult {
                    output: serde_json::to_string_pretty(&item).unwrap_or_default(),
                })
            }
            _ => {
                let items = self
                    .db
                    .list_fridge()
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok(ToolResult {
                    output: serde_json::to_string_pretty(&items).unwrap_or_default(),
                })
            }
        }
    }
}

impl Tool for SuggestTool {
    fn name(&self) -> &str {
        "fitness.suggest_meals"
    }
    fn execute(&self, _input: &str) -> Result<ToolResult, ToolError> {
        let today = local_today();
        let eaten: f64 = self
            .db
            .list_food_entries(&today)
            .unwrap_or_default()
            .iter()
            .map(|f| f.calories)
            .sum();
        let remaining = target(&self.db) - eaten;
        let recipes = self.db.list_recipes().unwrap_or_default();
        let fridge = self.db.list_fridge().unwrap_or_default();
        let meals = suggest_meals(&recipes, &fridge, remaining);
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&json!({
                "remaining_calories": remaining,
                "suggestions": meals,
            }))
            .unwrap_or_default(),
        })
    }
}

#[derive(Deserialize)]
struct WorkoutIn {
    name: String,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    duration_minutes: Option<i64>,
    #[serde(default)]
    sets: Vec<SetIn>,
}
#[derive(Deserialize)]
struct SetIn {
    exercise: String,
    #[serde(default)]
    reps: Option<i64>,
    #[serde(default)]
    weight: Option<f64>,
    #[serde(default)]
    duration_seconds: Option<i64>,
    #[serde(default)]
    rest_seconds: Option<i64>,
    #[serde(default)]
    distance: Option<f64>,
    #[serde(default)]
    notes: Option<String>,
}

impl Tool for LogWorkoutTool {
    fn name(&self) -> &str {
        "fitness.log_workout"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let p: WorkoutIn = parse_tool_json(input, "fitness.log_workout")?;
        let sets = p
            .sets
            .into_iter()
            .enumerate()
            .map(|(i, s)| WorkoutSet {
                id: String::new(),
                workout_id: String::new(),
                exercise: s.exercise,
                set_index: i as i64 + 1,
                reps: s.reps,
                weight: s.weight,
                duration_seconds: s.duration_seconds,
                rest_seconds: s.rest_seconds,
                distance: s.distance,
                notes: s.notes,
            })
            .collect();
        let (workout, prs) = self
            .db
            .save_workout(Workout {
                id: String::new(),
                name: p.name,
                date: p.date.unwrap_or_default(),
                notes: p.notes,
                duration_minutes: p.duration_minutes,
                created_at: 0,
                sets,
            })
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let pr_notes: Vec<String> = prs
            .iter()
            .map(|pr| match pr.previous {
                Some(prev) => format!(
                    "New PR — {}: {}{} (previous: {}{})",
                    pr.exercise, pr.value, pr.unit, prev, pr.unit
                ),
                None => format!("New PR — {}: {}{}", pr.exercise, pr.value, pr.unit),
            })
            .collect();
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&json!({
                "workout": workout,
                "prs": pr_notes,
            }))
            .unwrap_or_default(),
        })
    }
}

#[derive(Deserialize)]
struct ClimbIn {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    grade: Option<String>,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    attempts: Option<i64>,
    #[serde(default)]
    sent: Option<bool>,
    #[serde(default)]
    project: Option<bool>,
    #[serde(default)]
    style: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

impl Tool for ClimbingTool {
    fn name(&self) -> &str {
        "fitness.climbing_stats"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let p: ClimbIn = parse_tool_json(input, "fitness.climbing_stats").unwrap_or(ClimbIn {
            action: Some("stats".into()),
            name: None,
            grade: None,
            date: None,
            location: None,
            attempts: None,
            sent: None,
            project: None,
            style: None,
            notes: None,
        });
        match p.action.as_deref().unwrap_or("stats") {
            "log" => {
                let c = self
                    .db
                    .upsert_climb(Climb {
                        id: String::new(),
                        name: p.name.unwrap_or_else(|| "Problem".into()),
                        grade: p.grade.unwrap_or_else(|| "V0".into()),
                        date: p.date.unwrap_or_else(local_today),
                        location: p.location,
                        attempts: p.attempts.unwrap_or(1),
                        sent: p.sent.unwrap_or(false),
                        project: p.project.unwrap_or(false),
                        style: p.style,
                        notes: p.notes,
                        created_at: 0,
                    })
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok(ToolResult {
                    output: serde_json::to_string_pretty(&c).unwrap_or_default(),
                })
            }
            "list" => {
                let climbs = self
                    .db
                    .list_climbs()
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok(ToolResult {
                    output: serde_json::to_string_pretty(&climbs).unwrap_or_default(),
                })
            }
            _ => {
                let climbs = self
                    .db
                    .list_climbs()
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                let sends = climbs.iter().filter(|c| c.sent).count();
                let projects = climbs.iter().filter(|c| c.project).count();
                let highest = highest_grade(climbs.iter().filter(|c| c.sent).map(|c| c.grade.as_str()));
                Ok(ToolResult {
                    output: serde_json::to_string_pretty(&json!({
                        "total": climbs.len(),
                        "sends": sends,
                        "projects": projects,
                        "highest_send": highest,
                    }))
                    .unwrap_or_default(),
                })
            }
        }
    }
}

#[derive(Deserialize)]
struct WeightIn {
    kg: f64,
    #[serde(default)]
    date: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

impl Tool for LogWeightTool {
    fn name(&self) -> &str {
        "fitness.log_weight"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let p: WeightIn = parse_tool_json(input, "fitness.log_weight")?;
        if p.kg <= 0.0 || p.kg > 400.0 {
            return Err(ToolError::ExecutionFailed("kg must be a realistic weight".into()));
        }
        let e = self
            .db
            .upsert_weight_entry(WeightEntry {
                id: String::new(),
                date: p.date.filter(|d| !d.is_empty()).unwrap_or_else(local_today),
                kg: p.kg,
                notes: p.notes.filter(|n| !n.is_empty()),
                created_at: 0,
            })
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: serde_json::to_string_pretty(&e).unwrap_or_default(),
        })
    }
}
