use std::sync::Arc;

use buddy_core::TaskRunner;
use buddy_database::{
    local_today, Climb, Database, FoodEntry, FridgeItem, MoneyEntry, SocialIdea, StudySession,
    WeightEntry, Workout, WorkoutSet,
};
use buddy_memory::MemoryManager;
use buddy_plugins::{create_registry, tool_catalog_text};

fn runner() -> (TaskRunner, Arc<Database>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("buddy-look-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Arc::new(Database::open(&dir.join("buddy.db")).unwrap());
    let memory = Arc::new(MemoryManager::new(db.clone()));
    let registry = Arc::new(create_registry(
        db.clone(),
        memory,
        dir.display().to_string(),
    ));
    (TaskRunner::new(registry), db, dir)
}

#[test]
fn catalog_includes_look_tools() {
    let catalog = tool_catalog_text();
    for name in [
        "fitness.look",
        "money.list",
        "money.pots",
        "money.pot",
        "study.look",
        "study.upsert_topic",
        "study.upsert_assignment",
        "fitness.log_weight",
        "research.list",
        "research.get",
        "socials.look",
        "list_sparks",
    ] {
        assert!(catalog.contains(name), "missing {name} in catalog");
    }
}

#[test]
fn fitness_look_reads_food_workouts_weight() {
    let (runner, db, dir) = runner();
    let today = local_today();
    db.upsert_food_entry(FoodEntry {
        id: String::new(),
        name: "chicken rice".into(),
        quantity: 1.0,
        unit: "serving".into(),
        calories: 650.0,
        protein: 40.0,
        carbs: 70.0,
        fat: 18.0,
        date: today.clone(),
        meal_type: "lunch".into(),
        created_at: 0,
    })
    .unwrap();
    db.save_workout(Workout {
        id: String::new(),
        name: "Push".into(),
        date: today.clone(),
        notes: None,
        duration_minutes: None,
        created_at: 0,
        sets: vec![WorkoutSet {
            id: String::new(),
            workout_id: String::new(),
            exercise: "Bench Press".into(),
            set_index: 1,
            reps: Some(8),
            weight: Some(70.0),
            duration_seconds: None,
            rest_seconds: None,
            distance: None,
            notes: None,
        }],
    })
    .unwrap();
    db.upsert_weight_entry(WeightEntry {
        id: String::new(),
        date: today.clone(),
        kg: 82.4,
        notes: None,
        created_at: 0,
    })
    .unwrap();
    db.upsert_climb(Climb {
        id: String::new(),
        name: "Cave".into(),
        grade: "V10".into(),
        date: today.clone(),
        location: None,
        attempts: 3,
        sent: true,
        project: false,
        style: None,
        notes: None,
        created_at: 0,
    })
    .unwrap();
    db.upsert_fridge_item(FridgeItem {
        id: String::new(),
        name: "eggs".into(),
        quantity: 6.0,
        unit: "item".into(),
        category: "dairy".into(),
        expiry_date: None,
        created_at: 0,
        updated_at: 0,
    })
    .unwrap();

    let food = runner
        .run("fitness.look", r#"{"what":"food"}"#)
        .unwrap();
    assert!(food.output.contains("chicken rice"), "{}", food.output);

    let workouts = runner
        .run("fitness.look", r#"{"what":"workouts"}"#)
        .unwrap();
    assert!(workouts.output.contains("Bench Press"), "{}", workouts.output);
    assert!(workouts.output.contains("70"), "{}", workouts.output);

    let weight = runner
        .run("fitness.look", r#"{"what":"weight"}"#)
        .unwrap();
    assert!(weight.output.contains("82.4"), "{}", weight.output);

    let summary = runner.run("fitness.summary", "{}").unwrap();
    assert!(summary.output.contains("chicken rice"), "{}", summary.output);
    assert!(summary.output.contains("V10"), "{}", summary.output);

    let fridge = runner
        .run("fitness.look", r#"{"what":"fridge"}"#)
        .unwrap();
    assert!(fridge.output.contains("eggs"), "{}", fridge.output);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn money_list_and_study_look_and_research_and_sparks() {
    let (runner, db, dir) = runner();
    let today = local_today();
    let parts: Vec<i32> = today.split('-').filter_map(|p| p.parse().ok()).collect();
    let (year, month) = (parts[0], parts[1]);

    db.upsert_money_entry(MoneyEntry {
        id: String::new(),
        kind: "expense".into(),
        date: today.clone(),
        description: "lunch wrap".into(),
        category: "food".into(),
        amount_cents: 850,
        year: 0,
        month: 0,
        created_at: 0,
        updated_at: 0,
    })
    .unwrap();
    let listed = runner
        .run(
            "money.list",
            &format!(r#"{{"year":{year},"month":{month},"kind":"expense"}}"#),
        )
        .unwrap();
    assert!(listed.output.contains("lunch wrap"), "{}", listed.output);
    assert!(listed.output.contains("8.5"), "{}", listed.output);

    db.log_study_session(StudySession {
        id: String::new(),
        subject_id: None,
        topic_id: None,
        date: today.clone(),
        duration_minutes: 45,
        notes: Some("vectors".into()),
        created_at: 0,
    })
    .unwrap();
    let sessions = runner
        .run("study.look", r#"{"what":"sessions"}"#)
        .unwrap();
    assert!(sessions.output.contains("vectors"), "{}", sessions.output);

    db.ensure_research_session("conv-r1", "MLX vs llama", "which local model?")
        .unwrap();
    db.update_research_session(
        "conv-r1",
        buddy_database::UpdateResearchInput {
            summary: Some("MLX is faster on Apple silicon.".into()),
            findings: Some(vec!["use 4bit".into()]),
            ..Default::default()
        },
    )
    .unwrap();
    let research = runner.run("research.list", "{}").unwrap();
    assert!(research.output.contains("MLX vs llama"), "{}", research.output);
    let got = runner
        .run("research.get", r#"{"query":"local model"}"#)
        .unwrap();
    assert!(got.output.contains("Apple silicon"), "{}", got.output);

    db.create_spark("try a standing desk", &["general_life".into()], None)
        .unwrap();
    let sparks = runner.run("list_sparks", r#"{"status":"active"}"#).unwrap();
    assert!(sparks.output.contains("standing desk"), "{}", sparks.output);

    db.upsert_social_idea(SocialIdea {
        id: String::new(),
        body: "ship the look tools post".into(),
        platform: Some("linkedin".into()),
        thread_id: None,
        used_at: None,
        created_at: 0,
        updated_at: 0,
    })
    .unwrap();
    let ideas = runner
        .run("socials.look", r#"{"what":"ideas"}"#)
        .unwrap();
    assert!(ideas.output.contains("look tools"), "{}", ideas.output);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn study_upsert_topic_and_assignment_and_weight() {
    let (runner, _db, dir) = runner();
    let subject = runner
        .run("study.upsert_subject", r#"{"name":"English"}"#)
        .unwrap();
    assert!(subject.output.contains("English"), "{}", subject.output);

    let topic = runner
        .run(
            "study.upsert_topic",
            r#"{"name":"Hamlet","subject":"English","deadline":"2026-09-01"}"#,
        )
        .unwrap();
    assert!(topic.output.contains("Hamlet"), "{}", topic.output);

    let a1 = runner
        .run(
            "study.upsert_assignment",
            r#"{"title":"Read Act 1","subject":"English","kind":"homework"}"#,
        )
        .unwrap();
    assert!(a1.output.contains("Read Act 1"), "{}", a1.output);

    let topics = runner
        .run("study.look", r#"{"what":"topics"}"#)
        .unwrap();
    assert!(topics.output.contains("Hamlet"), "{}", topics.output);

    let wt = runner
        .run("fitness.log_weight", r#"{"kg":81.2}"#)
        .unwrap();
    assert!(wt.output.contains("81.2"), "{}", wt.output);
    let _ = std::fs::remove_dir_all(dir);
}
