use std::sync::Arc;

use buddy_calendar::CalendarService;
use buddy_core::{Tool, ToolRegistry};
use buddy_database::Database;
use buddy_plugins::SocialsPlugin;

fn open_db() -> (std::path::PathBuf, Arc<Database>) {
    let dir = std::env::temp_dir().join(format!("buddy-soc-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Arc::new(Database::open(&dir.join("buddy.db")).unwrap());
    (dir, db)
}

#[test]
fn commit_approved_writes_calendar_and_skips_rejected() {
    let (dir, db) = open_db();
    let calendar = Arc::new(CalendarService::with_db(db.clone()));
    let mut registry = ToolRegistry::new();
    SocialsPlugin::install(&mut registry, db.clone(), calendar);

    let plan = db
        .create_week_plan("2026-08-10", "Shipped auth", "built auth")
        .unwrap();
    let mut posts = plan.posts;
    assert!(!posts.iter().any(|p| p.platform == "github"));

    posts[0].status = "approved".into();
    posts[0].body = "Shipped auth this week.".into();
    posts[0].suggested_media = "Auth screenshot".into();
    posts[0].gather = vec!["Auth UI screenshot".into()];
    posts[0].category = "building".into();
    db.update_social_post(posts[0].clone()).unwrap();

    posts[1].status = "rejected".into();
    posts[1].body = "Skip me".into();
    db.update_social_post(posts[1].clone()).unwrap();

    let tool = registry.get("socials.commit_approved").unwrap();
    let out = tool
        .execute(&format!(r#"{{"plan_id":"{}"}}"#, plan.id))
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&out.output).unwrap();
    assert_eq!(parsed["created"], 1);

    let events = db
        .list_buddy_calendar_events(0, 2_000_000_000_000)
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].category, "social");
    let desc = events[0].description.clone().unwrap_or_default();
    assert!(desc.contains("Shipped auth this week."));
    assert!(desc.contains("Auth screenshot"));
    assert!(desc.contains("Auth UI screenshot"));
    assert!(desc.contains("Platform:"));
    assert!(!desc.contains("Skip me"));

    let refreshed = db.get_social_plan(&plan.id).unwrap();
    let rejected = refreshed.posts.iter().find(|p| p.status == "rejected").unwrap();
    assert!(rejected.calendar_event_id.is_none());
    let approved = refreshed.posts.iter().find(|p| p.id == posts[0].id).unwrap();
    assert!(approved.calendar_event_id.is_some());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn weekly_review_never_emits_github() {
    let catalog = buddy_plugins::tool_catalog_text();
    assert!(catalog.contains("socials.weekly_review"));
    assert!(catalog.contains("todo.list") || catalog.contains("todo.add"));
    assert!(catalog.contains("docs.search"));

    let (dir, db) = open_db();
    let plan = db.create_week_plan("2026-08-17", "", "").unwrap();
    assert_eq!(plan.posts.iter().filter(|p| p.platform == "linkedin").count(), 3);
    assert_eq!(plan.posts.iter().filter(|p| p.platform == "x").count(), 35);
    assert!(!plan.posts.iter().any(|p| p.platform == "github"));
    let _ = std::fs::remove_dir_all(dir);
}
