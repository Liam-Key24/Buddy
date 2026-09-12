use std::sync::Arc;

use buddy_core::{
    parse_tool_json, AfterExecute, AskKind, BuddyPlugin, FieldSpec, Permission, Safety, Tool,
    ToolDecl, ToolError, ToolResult, ToolSchema, ToolSpec,
};
use buddy_database::{
    catch_up_options, forecast_goal, local_today, propose_portfolio, Database, GoalForecast,
    UpsertGoal,
};
use serde::Deserialize;
use serde_json::json;

pub struct GoalsPlugin;

const GOAL_INTAKE_FIELDS: &[FieldSpec] = &[FieldSpec {
    name: "title",
    label: "goal",
    required: true,
    memory_keys: &[],
    ask_kind: AskKind::Text,
    choices: &[],
}];

const GOAL_INTAKE_SCHEMA: ToolSchema = ToolSchema {
    tool: "goal.intake",
    fields: GOAL_INTAKE_FIELDS,
};

fn extract_spending_cap(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    if !(lower.contains("i want") && lower.contains("below") && lower.contains("spend")) {
        return None;
    }
    let amount = lower
        .chars()
        .skip_while(|c| *c != '£' && !c.is_ascii_digit())
        .skip_while(|c| *c == '£')
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .collect::<String>()
        .replace(',', "");
    let amount: f64 = amount.parse().ok()?;
    Some(
        json!({
            "title": "Keep food spending under cap",
            "target_value": amount,
            "unit": "gbp",
            "current_value": 0.0,
            "progress_method": "amount",
            "assumptions": ["Spending records attach to this goal when relevant."],
        })
        .to_string(),
    )
}

const GOAL_SPECS: &[ToolSpec] = &[
    ToolSpec {
        name: "goal.intake",
        description: "create one or more goals without scheduling them",
        example: r#"goal.intake title="Climb V6" deadline=2026-12-01"#,
        schema: GOAL_INTAKE_SCHEMA,
        aliases: &[],
        rest_field: Some("title"),
        safety: Safety::Immediate,
        permission: Permission::None,
        respond: buddy_core::RespondMode::Passthrough,
        likely: &["i want to", "my goal", "by december"],
        extract: Some(extract_spending_cap),
        openai_properties_json: "",
    },
    ToolSpec::basic(
        "goal.look",
        "list active goals and forecasts",
        r#"goal.look"#,
        buddy_core::empty_schema("goal.look"),
    ),
    ToolSpec {
        name: "goal.propose_plan",
        description: "draft a portfolio calendar plan that needs approval",
        example: r#"goal.propose_plan weekly_capacity_minutes=600"#,
        schema: buddy_core::empty_schema("goal.propose_plan"),
        aliases: &[],
        rest_field: None,
        safety: Safety::ProposeFirst,
        permission: Permission::Confirm,
        respond: buddy_core::RespondMode::Passthrough,
        likely: &["plan my goals", "schedule my goals"],
        extract: None,
        openai_properties_json: "",
    },
    ToolSpec::basic(
        "goal.record_progress",
        "record progress and propose adaptations",
        r#"goal.record_progress id=<id> current_value=80"#,
        buddy_core::empty_schema("goal.record_progress"),
    ),
    ToolSpec::basic(
        "goal.promote_spark",
        "promote a Spark into a goal",
        r#"goal.promote_spark spark_id=<id>"#,
        buddy_core::empty_schema("goal.promote_spark"),
    ),
];

struct IntakeTool {
    db: Arc<Database>,
}
struct LookTool {
    db: Arc<Database>,
}
struct ProposeTool {
    db: Arc<Database>,
}
struct ProgressTool {
    db: Arc<Database>,
}
struct PromoteTool {
    db: Arc<Database>,
}

impl BuddyPlugin for GoalsPlugin {
    fn id(&self) -> &'static str {
        "goals"
    }

    fn tools(&self, db: Arc<Database>) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(IntakeTool { db: db.clone() }),
            Arc::new(LookTool { db: db.clone() }),
            Arc::new(ProposeTool { db: db.clone() }),
            Arc::new(ProgressTool { db: db.clone() }),
            Arc::new(PromoteTool { db }),
        ]
    }

    fn tool_decls(&self) -> &'static [ToolDecl] {
        &[ToolDecl {
            name: "goal.intake",
            planner_line: "goal.intake: create goals from structured fields. Never commits calendar time.",
        }]
    }

    fn tool_specs(&self) -> &'static [ToolSpec] {
        GOAL_SPECS
    }

    fn after_execute_hint(&self, _tool_name: &str) -> AfterExecute {
        AfterExecute::None
    }
}

#[derive(Deserialize, Default)]
struct IntakeIn {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    goals: Option<Vec<UpsertGoal>>,
    #[serde(default)]
    deadline: Option<String>,
    #[serde(default)]
    target_value: Option<f64>,
    #[serde(default)]
    current_value: Option<f64>,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    progress_method: Option<String>,
    #[serde(default)]
    motivation: Option<String>,
    #[serde(default)]
    assumptions: Option<Vec<String>>,
}

impl Tool for IntakeTool {
    fn name(&self) -> &str {
        "goal.intake"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: IntakeIn = parse_tool_json(input, "goal.intake").unwrap_or_default();
        let mut drafts = parsed.goals.unwrap_or_default();
        if drafts.is_empty() {
            let title = parsed.title.unwrap_or_default();
            if title.trim().is_empty() {
                return Err(ToolError::ExecutionFailed("title or goals required".into()));
            }
            drafts.push(UpsertGoal {
                title,
                deadline: parsed.deadline,
                target_value: parsed.target_value,
                current_value: parsed.current_value,
                unit: parsed.unit,
                progress_method: parsed.progress_method,
                motivation: parsed.motivation,
                assumptions: parsed.assumptions.unwrap_or_default(),
                ..Default::default()
            });
        }
        let mut created = Vec::new();
        let mut questions = Vec::new();
        for draft in drafts {
            let mut assumptions = draft.assumptions.clone();
            if assumptions.is_empty() {
                assumptions.push("Progress is linear unless you say otherwise.".into());
            }
            if draft.current_value.is_none() && draft.target_value.is_some() {
                questions.push(format!("Current position for '{}'?", draft.title));
            }
            if draft.deadline.is_none() {
                questions.push(format!("Hard deadline for '{}'?", draft.title));
            }
            let mut input = draft;
            input.assumptions = assumptions;
            let goal = self
                .db
                .upsert_goal(input)
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            let _ = self.db.add_goal_history(&goal.id, "created", &goal.title);
            created.push(goal);
        }
        Ok(ToolResult {
            output: json!({
                "status": "captured",
                "goals": created,
                "questions": questions,
                "scheduled": false,
                "note": "No calendar time was reserved. Use goal.propose_plan when ready."
            })
            .to_string(),
        })
    }
}

impl Tool for LookTool {
    fn name(&self) -> &str {
        "goal.look"
    }
    fn execute(&self, _input: &str) -> Result<ToolResult, ToolError> {
        let today = local_today();
        let goals = self
            .db
            .list_goals(Some("active"))
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let rows: Vec<_> = goals
            .iter()
            .map(|g| {
                let forecast = forecast_goal(g, &today);
                json!({
                    "id": g.id,
                    "title": g.title,
                    "deadline": g.deadline,
                    "forecast": forecast.as_str(),
                    "current_value": g.current_value,
                    "target_value": g.target_value,
                    "unit": g.unit,
                    "source_spark_id": g.source_spark_id,
                })
            })
            .collect();
        Ok(ToolResult {
            output: json!({ "goals": rows, "today": today }).to_string(),
        })
    }
}

#[derive(Deserialize, Default)]
struct ProposeIn {
    #[serde(default)]
    weekly_capacity_minutes: Option<i64>,
    #[serde(default)]
    apply: Option<bool>,
}

impl Tool for ProposeTool {
    fn name(&self) -> &str {
        "goal.propose_plan"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: ProposeIn = parse_tool_json(input, "goal.propose_plan").unwrap_or_default();
        let goals = self
            .db
            .list_goals(Some("active"))
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let plan = propose_portfolio(&goals, parsed.weekly_capacity_minutes.unwrap_or(600));
        let apply = parsed.apply.unwrap_or(false);
        let status = if apply { "planned" } else { "proposed" };
        let actions = self
            .db
            .insert_planned_actions(&plan.actions, status)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: json!({
                "status": if apply { "committed" } else { "proposed" },
                "needs_approval": !apply,
                "assumptions": plan.assumptions,
                "questions": plan.questions,
                "weekly_minutes_requested": plan.weekly_minutes_requested,
                "weekly_capacity_minutes": plan.weekly_capacity_minutes,
                "actions": actions,
            })
            .to_string(),
        })
    }
}

#[derive(Deserialize, Default)]
struct ProgressIn {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    current_value: Option<f64>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    source: Option<String>,
}

impl Tool for ProgressTool {
    fn name(&self) -> &str {
        "goal.record_progress"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: ProgressIn = parse_tool_json(input, "goal.record_progress").unwrap_or_default();
        let id = parsed
            .id
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| ToolError::ExecutionFailed("id required".into()))?;
        let value = parsed
            .current_value
            .ok_or_else(|| ToolError::ExecutionFailed("current_value required".into()))?;
        let goal = self
            .db
            .set_goal_progress(&id, value)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let _ = self.db.add_progress_signal(
            &id,
            parsed.source.as_deref().unwrap_or("user"),
            Some(value),
            goal.unit.as_deref(),
            parsed.notes.as_deref(),
            true,
        );
        let forecast = GoalForecast::parse(&goal.current_forecast);
        let options = catch_up_options(forecast);
        let mut released = 0;
        if forecast == GoalForecast::Ahead {
            released = self
                .db
                .release_flexible_for_goal(&id)
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        }
        let adaptations: Vec<String> = options.iter().map(|o| o.summary.clone()).collect();
        let _ = self.db.add_review(
            Some(&id),
            "previous forecast",
            &format!("current={value}"),
            forecast.as_str(),
            &adaptations,
            None,
        );
        Ok(ToolResult {
            output: json!({
                "goal": goal,
                "forecast": forecast.as_str(),
                "adapt_options": options,
                "released_sessions": released,
                "released_stays_free": true,
                "applied": false,
            })
            .to_string(),
        })
    }
}

#[derive(Deserialize, Default)]
struct PromoteIn {
    spark_id: Option<String>,
}

impl Tool for PromoteTool {
    fn name(&self) -> &str {
        "goal.promote_spark"
    }
    fn execute(&self, input: &str) -> Result<ToolResult, ToolError> {
        let parsed: PromoteIn = parse_tool_json(input, "goal.promote_spark").unwrap_or_default();
        let spark_id = parsed
            .spark_id
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| ToolError::ExecutionFailed("spark_id required".into()))?;
        let spark = self
            .db
            .get_spark(&spark_id)
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let goal = self
            .db
            .upsert_goal(UpsertGoal {
                title: spark.content.chars().take(80).collect(),
                desired_outcome: Some(spark.content.clone()),
                source_spark_id: Some(spark_id.clone()),
                assumptions: vec!["Promoted from a Spark — not scheduled yet.".into()],
                ..Default::default()
            })
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        Ok(ToolResult {
            output: json!({
                "goal": goal,
                "spark_id": spark_id,
                "scheduled": false,
            })
            .to_string(),
        })
    }
}
