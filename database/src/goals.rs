//! Goal-to-calendar records and deterministic forecasts.
//!
//! Todos stay todos. `workspace_profiles.goals` is unrelated workspace notes.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{add_days_to_date, chrono_now, local_today, Database, DbError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalForecast {
    Ahead,
    OnTrack,
    AtRisk,
    Behind,
    Blocked,
    Complete,
}

impl GoalForecast {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ahead => "ahead",
            Self::OnTrack => "on_track",
            Self::AtRisk => "at_risk",
            Self::Behind => "behind",
            Self::Blocked => "blocked",
            Self::Complete => "complete",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw {
            "ahead" => Self::Ahead,
            "at_risk" => Self::AtRisk,
            "behind" => Self::Behind,
            "blocked" => Self::Blocked,
            "complete" => Self::Complete,
            _ => Self::OnTrack,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub desired_outcome: Option<String>,
    #[serde(default)]
    pub motivation: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub deadline: Option<String>,
    pub priority: String,
    pub status: String,
    #[serde(default)]
    pub progress_method: Option<String>,
    pub current_forecast: String,
    #[serde(default)]
    pub protected_constraints: Vec<String>,
    #[serde(default)]
    pub source_spark_id: Option<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub current_value: Option<f64>,
    #[serde(default)]
    pub target_value: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpsertGoal {
    #[serde(default)]
    pub id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub desired_outcome: Option<String>,
    #[serde(default)]
    pub motivation: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub deadline: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub progress_method: Option<String>,
    #[serde(default)]
    pub protected_constraints: Vec<String>,
    #[serde(default)]
    pub source_spark_id: Option<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub current_value: Option<f64>,
    #[serde(default)]
    pub target_value: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub id: String,
    pub goal_id: String,
    pub outcome: String,
    #[serde(default)]
    pub target_date: Option<String>,
    pub ordering: i64,
    #[serde(default)]
    pub completion_rule: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedAction {
    pub id: String,
    pub goal_id: String,
    #[serde(default)]
    pub milestone_id: Option<String>,
    pub description: String,
    #[serde(default)]
    pub estimated_minutes: Option<i64>,
    pub flexibility: String,
    pub priority: String,
    pub status: String,
    #[serde(default)]
    pub calendar_event_id: Option<String>,
    #[serde(default)]
    pub deadline: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressSignal {
    pub id: String,
    pub goal_id: String,
    pub source: String,
    #[serde(default)]
    pub measurement: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
    pub recorded_at: i64,
    pub confidence: f64,
    #[serde(default)]
    pub notes: Option<String>,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalReview {
    pub id: String,
    pub goal_id: Option<String>,
    pub expected_position: Option<String>,
    pub actual_position: Option<String>,
    pub forecast: Option<String>,
    pub recommended_adaptations: Vec<String>,
    pub user_decision: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptOption {
    pub kind: String,
    pub summary: String,
    pub increases_workload: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioPlan {
    pub assumptions: Vec<String>,
    pub questions: Vec<String>,
    pub weekly_minutes_requested: i64,
    pub weekly_capacity_minutes: i64,
    pub actions: Vec<PlannedActionDraft>,
    pub needs_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedActionDraft {
    pub goal_id: String,
    pub description: String,
    pub estimated_minutes: i64,
    pub flexibility: String,
    pub priority: String,
}

fn json_list(raw: Option<String>) -> Vec<String> {
    raw.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn list_json(items: &[String]) -> String {
    serde_json::to_string(items).unwrap_or_else(|_| "[]".into())
}

/// Deterministic forecast from elapsed time vs measured progress.
pub fn forecast_goal(goal: &Goal, today: &str) -> GoalForecast {
    if goal.status == "complete" {
        return GoalForecast::Complete;
    }
    if goal.status == "blocked" {
        return GoalForecast::Blocked;
    }
    let Some(target) = goal.target_value.filter(|t| *t > 0.0) else {
        return GoalForecast::OnTrack;
    };
    let current = goal.current_value.unwrap_or(0.0);
    if current >= target {
        return if goal.status == "complete" {
            GoalForecast::Complete
        } else {
            GoalForecast::Ahead
        };
    }
    let Some(deadline) = goal.deadline.as_deref() else {
        return GoalForecast::OnTrack;
    };
    let start = goal.start_date.as_deref().unwrap_or(today);
    let total = days_between(start, deadline).unwrap_or(0) as f64;
    let elapsed = days_between(start, today).unwrap_or(0) as f64;
    if total <= 0.0 {
        return GoalForecast::AtRisk;
    }
    let time_frac = (elapsed / total).clamp(0.0, 1.2);
    let progress_frac = (current / target).clamp(0.0, 1.0);
    let gap = time_frac - progress_frac;
    if gap > 0.20 {
        GoalForecast::Behind
    } else if gap > 0.08 {
        GoalForecast::AtRisk
    } else if progress_frac > time_frac + 0.08 {
        GoalForecast::Ahead
    } else {
        GoalForecast::OnTrack
    }
}

fn days_between(from: &str, to: &str) -> Option<i32> {
    let a = crate::ymd_to_ordinal_pub(parse_ymd(from)?.0, parse_ymd(from)?.1, parse_ymd(from)?.2)?;
    let b = crate::ymd_to_ordinal_pub(parse_ymd(to)?.0, parse_ymd(to)?.1, parse_ymd(to)?.2)?;
    Some(b - a)
}

fn parse_ymd(date: &str) -> Option<(i32, i32, i32)> {
    let parts: Vec<i32> = date.split('-').filter_map(|p| p.parse().ok()).collect();
    if parts.len() == 3 {
        Some((parts[0], parts[1], parts[2]))
    } else {
        None
    }
}

/// Plan several goals against one weekly capacity. Never fill all free time.
pub fn propose_portfolio(goals: &[Goal], weekly_capacity_minutes: i64) -> PortfolioPlan {
    let usable = (weekly_capacity_minutes as f64 * 0.7).round() as i64;
    let active: Vec<&Goal> = goals
        .iter()
        .filter(|g| g.status == "active")
        .collect();
    let mut assumptions = vec![
        format!("Weekly planning uses {usable} of {weekly_capacity_minutes} available minutes."),
        "Released time stays free unless you approve a reallocation.".into(),
    ];
    let mut questions = Vec::new();
    let mut actions = Vec::new();
    if active.is_empty() {
        return PortfolioPlan {
            assumptions,
            questions,
            weekly_minutes_requested: 0,
            weekly_capacity_minutes,
            actions,
            needs_approval: true,
        };
    }
    let weight_sum: i64 = active
        .iter()
        .map(|g| priority_weight(&g.priority))
        .sum::<i64>()
        .max(1);
    for goal in &active {
        if goal.current_value.is_none() && goal.target_value.is_some() {
            questions.push(format!("What is your current position for '{}'?", goal.title));
        }
        if goal.deadline.is_none() {
            questions.push(format!("Does '{}' have a hard deadline?", goal.title));
        }
        let share = usable * priority_weight(&goal.priority) / weight_sum;
        let minutes = share.max(30);
        actions.push(PlannedActionDraft {
            goal_id: goal.id.clone(),
            description: format!("Work session for {}", goal.title),
            estimated_minutes: minutes,
            flexibility: "flexible".into(),
            priority: goal.priority.clone(),
        });
        assumptions.push(format!(
            "'{}' gets about {minutes} minutes this week.",
            goal.title
        ));
    }
    let requested: i64 = actions.iter().map(|a| a.estimated_minutes).sum();
    PortfolioPlan {
        assumptions,
        questions,
        weekly_minutes_requested: requested,
        weekly_capacity_minutes,
        actions,
        needs_approval: true,
    }
}

fn priority_weight(priority: &str) -> i64 {
    match priority {
        "high" => 3,
        "low" => 1,
        _ => 2,
    }
}

pub fn catch_up_options(forecast: GoalForecast) -> Vec<AdaptOption> {
    match forecast {
        GoalForecast::Behind | GoalForecast::AtRisk => vec![
            AdaptOption {
                kind: "extra_session".into(),
                summary: "Add one extra session this week".into(),
                increases_workload: true,
            },
            AdaptOption {
                kind: "longer_session".into(),
                summary: "Lengthen existing flexible sessions".into(),
                increases_workload: true,
            },
            AdaptOption {
                kind: "reduce_scope".into(),
                summary: "Reduce scope or move the deadline".into(),
                increases_workload: false,
            },
        ],
        GoalForecast::Ahead => vec![
            AdaptOption {
                kind: "release_time".into(),
                summary: "Release unused sessions back to free time".into(),
                increases_workload: false,
            },
            AdaptOption {
                kind: "advance_other".into(),
                summary: "Suggest advancing another goal — not automatic".into(),
                increases_workload: false,
            },
        ],
        _ => Vec::new(),
    }
}

/// Ahead of plan: flexible future work is released, not refilled.
pub fn release_flexible_actions(actions: &mut [PlannedAction]) -> usize {
    let mut n = 0;
    for action in actions {
        if action.status == "planned" && action.flexibility == "flexible" {
            action.status = "released".into();
            n += 1;
        }
    }
    n
}

impl Database {
    pub fn upsert_goal(&self, input: UpsertGoal) -> Result<Goal, DbError> {
        let conn = self.conn.lock().unwrap();
        let now = chrono_now();
        let id = input
            .id
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let start = input
            .start_date
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(local_today);
        conn.execute(
            "INSERT INTO buddy_goals (
                id, title, desired_outcome, motivation, start_date, deadline, priority, status,
                progress_method, current_forecast, protected_constraints, source_spark_id,
                assumptions_json, current_value, target_value, unit, created_at, updated_at
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,'active',?8,'on_track',?9,?10,?11,?12,?13,?14,?15,?15)
            ON CONFLICT(id) DO UPDATE SET
                title=excluded.title,
                desired_outcome=excluded.desired_outcome,
                motivation=excluded.motivation,
                start_date=excluded.start_date,
                deadline=excluded.deadline,
                priority=excluded.priority,
                progress_method=excluded.progress_method,
                protected_constraints=excluded.protected_constraints,
                source_spark_id=excluded.source_spark_id,
                assumptions_json=excluded.assumptions_json,
                current_value=excluded.current_value,
                target_value=excluded.target_value,
                unit=excluded.unit,
                updated_at=excluded.updated_at",
            params![
                id,
                input.title,
                input.desired_outcome,
                input.motivation,
                start,
                input.deadline,
                input.priority.unwrap_or_else(|| "medium".into()),
                input.progress_method,
                list_json(&input.protected_constraints),
                input.source_spark_id,
                list_json(&input.assumptions),
                input.current_value,
                input.target_value,
                input.unit,
                now,
            ],
        )?;
        drop(conn);
        self.get_goal(&id)
    }

    pub fn get_goal(&self, id: &str) -> Result<Goal, DbError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, title, desired_outcome, motivation, start_date, deadline, priority, status,
                    progress_method, current_forecast, protected_constraints, source_spark_id,
                    assumptions_json, current_value, target_value, unit, created_at, updated_at
             FROM buddy_goals WHERE id=?1",
            params![id],
            row_to_goal,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NotFound(id.into()),
            other => DbError::Sqlite(other),
        })
    }

    pub fn list_goals(&self, status: Option<&str>) -> Result<Vec<Goal>, DbError> {
        let conn = self.conn.lock().unwrap();
        let sql = if status.is_some() {
            "SELECT id, title, desired_outcome, motivation, start_date, deadline, priority, status,
                    progress_method, current_forecast, protected_constraints, source_spark_id,
                    assumptions_json, current_value, target_value, unit, created_at, updated_at
             FROM buddy_goals WHERE status=?1 ORDER BY updated_at DESC"
        } else {
            "SELECT id, title, desired_outcome, motivation, start_date, deadline, priority, status,
                    progress_method, current_forecast, protected_constraints, source_spark_id,
                    assumptions_json, current_value, target_value, unit, created_at, updated_at
             FROM buddy_goals ORDER BY updated_at DESC"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = if let Some(status) = status {
            stmt.query_map(params![status], row_to_goal)?
                .collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map([], row_to_goal)?
                .collect::<Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    pub fn set_goal_forecast(&self, id: &str, forecast: GoalForecast) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE buddy_goals SET current_forecast=?1, updated_at=?2 WHERE id=?3",
            params![forecast.as_str(), chrono_now(), id],
        )?;
        Ok(())
    }

    pub fn set_goal_progress(&self, id: &str, current_value: f64) -> Result<Goal, DbError> {
        {
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "UPDATE buddy_goals SET current_value=?1, updated_at=?2 WHERE id=?3",
                params![current_value, chrono_now(), id],
            )?;
        }
        let mut goal = self.get_goal(id)?;
        let forecast = forecast_goal(&goal, &local_today());
        self.set_goal_forecast(id, forecast)?;
        goal.current_forecast = forecast.as_str().into();
        goal.current_value = Some(current_value);
        Ok(goal)
    }

    pub fn add_progress_signal(
        &self,
        goal_id: &str,
        source: &str,
        measurement: Option<f64>,
        unit: Option<&str>,
        notes: Option<&str>,
        confirmed: bool,
    ) -> Result<ProgressSignal, DbError> {
        let now = chrono_now();
        let id = Uuid::new_v4().to_string();
        {
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO buddy_progress_signals (
                    id, goal_id, source, measurement, unit, recorded_at, confidence, notes, confirmed, created_at
                ) VALUES (?1,?2,?3,?4,?5,?6,1.0,?7,?8,?6)",
                params![
                    id,
                    goal_id,
                    source,
                    measurement,
                    unit,
                    now,
                    notes,
                    if confirmed { 1 } else { 0 }
                ],
            )?;
        }
        if let Some(value) = measurement {
            let _ = self.set_goal_progress(goal_id, value);
        }
        self.add_goal_history(goal_id, "progress", notes.unwrap_or(""))?;
        Ok(ProgressSignal {
            id,
            goal_id: goal_id.into(),
            source: source.into(),
            measurement,
            unit: unit.map(|s| s.into()),
            recorded_at: now,
            confidence: 1.0,
            notes: notes.map(|s| s.into()),
            confirmed,
        })
    }

    pub fn insert_planned_actions(
        &self,
        drafts: &[PlannedActionDraft],
        status: &str,
    ) -> Result<Vec<PlannedAction>, DbError> {
        let now = chrono_now();
        let mut out = Vec::new();
        let conn = self.conn.lock().unwrap();
        for draft in drafts {
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO buddy_planned_actions (
                    id, goal_id, milestone_id, description, estimated_minutes, energy, frequency,
                    earliest_start, deadline, flexibility, priority, calendar_event_id, status,
                    created_at, updated_at
                ) VALUES (?1,?2,NULL,?3,?4,NULL,'weekly',NULL,NULL,?5,?6,NULL,?7,?8,?8)",
                params![
                    id,
                    draft.goal_id,
                    draft.description,
                    draft.estimated_minutes,
                    draft.flexibility,
                    draft.priority,
                    status,
                    now
                ],
            )?;
            out.push(PlannedAction {
                id,
                goal_id: draft.goal_id.clone(),
                milestone_id: None,
                description: draft.description.clone(),
                estimated_minutes: Some(draft.estimated_minutes),
                flexibility: draft.flexibility.clone(),
                priority: draft.priority.clone(),
                status: status.into(),
                calendar_event_id: None,
                deadline: None,
            });
        }
        Ok(out)
    }

    pub fn list_planned_actions(&self, goal_id: Option<&str>) -> Result<Vec<PlannedAction>, DbError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = if goal_id.is_some() {
            conn.prepare(
                "SELECT id, goal_id, milestone_id, description, estimated_minutes, flexibility,
                        priority, status, calendar_event_id, deadline
                 FROM buddy_planned_actions WHERE goal_id=?1 ORDER BY created_at",
            )?
        } else {
            conn.prepare(
                "SELECT id, goal_id, milestone_id, description, estimated_minutes, flexibility,
                        priority, status, calendar_event_id, deadline
                 FROM buddy_planned_actions ORDER BY created_at",
            )?
        };
        let map = |row: &rusqlite::Row| -> Result<PlannedAction, rusqlite::Error> {
            Ok(PlannedAction {
                id: row.get(0)?,
                goal_id: row.get(1)?,
                milestone_id: row.get(2)?,
                description: row.get(3)?,
                estimated_minutes: row.get(4)?,
                flexibility: row.get(5)?,
                priority: row.get(6)?,
                status: row.get(7)?,
                calendar_event_id: row.get(8)?,
                deadline: row.get(9)?,
            })
        };
        let rows = if let Some(id) = goal_id {
            stmt.query_map(params![id], map)?.collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map([], map)?.collect::<Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    pub fn release_flexible_for_goal(&self, goal_id: &str) -> Result<usize, DbError> {
        let mut actions = self.list_planned_actions(Some(goal_id))?;
        let n = release_flexible_actions(&mut actions);
        let conn = self.conn.lock().unwrap();
        let now = chrono_now();
        for action in actions.iter().filter(|a| a.status == "released") {
            conn.execute(
                "UPDATE buddy_planned_actions SET status='released', updated_at=?1 WHERE id=?2",
                params![now, action.id],
            )?;
        }
        Ok(n)
    }

    pub fn add_review(
        &self,
        goal_id: Option<&str>,
        expected: &str,
        actual: &str,
        forecast: &str,
        adaptations: &[String],
        decision: Option<&str>,
    ) -> Result<GoalReview, DbError> {
        let id = Uuid::new_v4().to_string();
        let now = chrono_now();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO buddy_reviews (
                id, goal_id, expected_position, actual_position, forecast, blockers_json,
                estimation_errors, recommended_adaptations_json, user_decision, created_at
            ) VALUES (?1,?2,?3,?4,?5,'[]',NULL,?6,?7,?8)",
            params![
                id,
                goal_id,
                expected,
                actual,
                forecast,
                list_json(adaptations),
                decision,
                now
            ],
        )?;
        Ok(GoalReview {
            id,
            goal_id: goal_id.map(|s| s.into()),
            expected_position: Some(expected.into()),
            actual_position: Some(actual.into()),
            forecast: Some(forecast.into()),
            recommended_adaptations: adaptations.to_vec(),
            user_decision: decision.map(|s| s.into()),
            created_at: now,
        })
    }

    pub fn add_goal_history(&self, goal_id: &str, kind: &str, payload: &str) -> Result<(), DbError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO buddy_goal_history (id, goal_id, kind, payload, created_at)
             VALUES (?1,?2,?3,?4,?5)",
            params![Uuid::new_v4().to_string(), goal_id, kind, payload, chrono_now()],
        )?;
        Ok(())
    }

    pub fn insert_milestone(
        &self,
        goal_id: &str,
        outcome: &str,
        target_date: Option<&str>,
        ordering: i64,
    ) -> Result<Milestone, DbError> {
        let id = Uuid::new_v4().to_string();
        let now = chrono_now();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO buddy_milestones (
                id, goal_id, outcome, target_date, ordering, completion_rule, dependencies_json,
                status, created_at, updated_at
            ) VALUES (?1,?2,?3,?4,?5,NULL,'[]','open',?6,?6)",
            params![id, goal_id, outcome, target_date, ordering, now],
        )?;
        Ok(Milestone {
            id,
            goal_id: goal_id.into(),
            outcome: outcome.into(),
            target_date: target_date.map(|s| s.into()),
            ordering,
            completion_rule: None,
            status: "open".into(),
        })
    }

    pub fn next_week_date() -> String {
        add_days_to_date(&local_today(), 7).unwrap_or_else(local_today)
    }
}

fn row_to_goal(row: &rusqlite::Row<'_>) -> Result<Goal, rusqlite::Error> {
    Ok(Goal {
        id: row.get(0)?,
        title: row.get(1)?,
        desired_outcome: row.get(2)?,
        motivation: row.get(3)?,
        start_date: row.get(4)?,
        deadline: row.get(5)?,
        priority: row.get(6)?,
        status: row.get(7)?,
        progress_method: row.get(8)?,
        current_forecast: row.get(9)?,
        protected_constraints: json_list(row.get(10)?),
        source_spark_id: row.get(11)?,
        assumptions: json_list(row.get(12)?),
        current_value: row.get(13)?,
        target_value: row.get(14)?,
        unit: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn goal(title: &str, current: f64, target: f64, start: &str, deadline: &str) -> Goal {
        Goal {
            id: title.into(),
            title: title.into(),
            desired_outcome: None,
            motivation: None,
            start_date: Some(start.into()),
            deadline: Some(deadline.into()),
            priority: "medium".into(),
            status: "active".into(),
            progress_method: Some("pages".into()),
            current_forecast: "on_track".into(),
            protected_constraints: vec![],
            source_spark_id: None,
            assumptions: vec![],
            current_value: Some(current),
            target_value: Some(target),
            unit: Some("pages".into()),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn forecast_behind_and_ahead() {
        let behind = goal("book", 20.0, 200.0, "2026-01-01", "2026-02-01");
        assert_eq!(forecast_goal(&behind, "2026-01-25"), GoalForecast::Behind);
        let ahead = goal("book", 180.0, 200.0, "2026-01-01", "2026-02-01");
        assert_eq!(forecast_goal(&ahead, "2026-01-10"), GoalForecast::Ahead);
    }

    #[test]
    fn portfolio_splits_capacity_and_keeps_free_time() {
        let goals = vec![
            goal("climb V6", 0.0, 1.0, "2026-09-01", "2026-12-01"),
            goal("save", 0.0, 2000.0, "2026-09-01", "2026-12-01"),
            goal("read", 0.0, 2.0, "2026-09-01", "2026-12-01"),
        ];
        let plan = propose_portfolio(&goals, 600);
        assert_eq!(plan.actions.len(), 3);
        assert!(plan.weekly_minutes_requested <= 420);
        assert!(plan.needs_approval);
        assert!(plan.assumptions.iter().any(|a| a.contains("stays free")));
    }

    #[test]
    fn released_time_is_not_reallocated() {
        let mut actions = vec![PlannedAction {
            id: "1".into(),
            goal_id: "g".into(),
            milestone_id: None,
            description: "session".into(),
            estimated_minutes: Some(60),
            flexibility: "flexible".into(),
            priority: "medium".into(),
            status: "planned".into(),
            calendar_event_id: None,
            deadline: None,
        }];
        assert_eq!(release_flexible_actions(&mut actions), 1);
        assert_eq!(actions[0].status, "released");
        assert!(catch_up_options(GoalForecast::Ahead)
            .iter()
            .any(|o| o.kind == "release_time" && !o.increases_workload));
    }
}
