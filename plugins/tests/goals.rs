use std::sync::Arc;

use buddy_core::Route;
use buddy_database::{forecast_goal, propose_portfolio, Database, Goal, GoalForecast, UpsertGoal};
use buddy_memory::MemoryManager;
use buddy_plugins::PluginManager;

fn surface() -> (std::path::PathBuf, buddy_plugins::PluginSurface) {
    let dir = std::env::temp_dir().join(format!("buddy-goals-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Arc::new(Database::open(&dir.join("buddy.db")).unwrap());
    let memory = Arc::new(MemoryManager::new(db.clone()));
    let mgr = PluginManager::bootstrap(db, memory, dir.display().to_string());
    let (_registry, surface) = mgr.finish();
    (dir, surface)
}

#[test]
fn spending_record_does_not_extract_a_goal() {
    let (_dir, surface) = surface();
    match surface.route("I spent £20 on dinner") {
        Route::Tools(jobs) => assert!(jobs.iter().all(|j| j.tool != "goal.intake")),
        Route::Miss => {}
    }
}

#[test]
fn spending_cap_extracts_goal_intake() {
    let (_dir, surface) = surface();
    match surface.route("I want to keep food spending below £250 this month") {
        Route::Tools(jobs) => {
            assert!(jobs.iter().any(|j| j.tool == "goal.intake"));
        }
        other => panic!("expected extract, got {other:?}"),
    }
}

#[test]
fn multi_goal_portfolio_needs_approval_and_leaves_free_time() {
    let dir = std::env::temp_dir().join(format!("buddy-goals-db-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Database::open(&dir.join("buddy.db")).unwrap();
    let goals: Vec<Goal> = ["Climb V6", "Save £2000", "Read two books"]
        .into_iter()
        .map(|title| {
            db.upsert_goal(UpsertGoal {
                title: title.into(),
                deadline: Some("2026-12-01".into()),
                target_value: Some(1.0),
                current_value: Some(0.0),
                ..Default::default()
            })
            .unwrap()
        })
        .collect();
    let plan = propose_portfolio(&goals, 600);
    assert_eq!(plan.actions.len(), 3);
    assert!(plan.needs_approval);
    assert!(plan.weekly_minutes_requested < 600);
}

#[test]
fn behind_offers_workload_options_ahead_does_not() {
    let behind = Goal {
        id: "b".into(),
        title: "Read".into(),
        desired_outcome: None,
        motivation: None,
        start_date: Some("2026-01-01".into()),
        deadline: Some("2026-02-01".into()),
        priority: "medium".into(),
        status: "active".into(),
        progress_method: Some("pages".into()),
        current_forecast: "on_track".into(),
        protected_constraints: vec![],
        source_spark_id: None,
        assumptions: vec![],
        current_value: Some(20.0),
        target_value: Some(200.0),
        unit: Some("pages".into()),
        created_at: 0,
        updated_at: 0,
    };
    assert_eq!(forecast_goal(&behind, "2026-01-25"), GoalForecast::Behind);
    assert!(buddy_database::catch_up_options(GoalForecast::Behind)
        .iter()
        .any(|o| o.increases_workload));
    assert!(buddy_database::catch_up_options(GoalForecast::Ahead)
        .iter()
        .all(|o| !o.increases_workload));
}

#[test]
fn canonical_goal_look() {
    let (_dir, surface) = surface();
    match surface.route("goal.look") {
        Route::Tools(jobs) => assert_eq!(jobs[0].tool, "goal.look"),
        other => panic!("{other:?}"),
    }
}
