use std::sync::Arc;

use buddy_core::{AfterExecute, TaskRunner};
use buddy_database::Database;
use buddy_memory::MemoryManager;
use buddy_plugins::{after_execute_hint, create_registry, tool_catalog_text};

#[test]
fn catalog_includes_docs_write_tools() {
    let catalog = tool_catalog_text();
    assert!(catalog.contains("docs.upsert"));
    assert!(catalog.contains("docs.delete"));
    assert!(catalog.contains("docs.get"));
    assert!(catalog.contains("docs.format"));
    assert!(catalog.contains("docs.patch"));
}

#[test]
fn upsert_get_by_title_delete_and_hint() {
    let dir = std::env::temp_dir().join(format!("buddy-docs-tool-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = Arc::new(Database::open(&dir.join("buddy.db")).unwrap());
    let memory = Arc::new(MemoryManager::new(db.clone()));
    let registry = Arc::new(create_registry(db, memory, dir.display().to_string()));
    let runner = TaskRunner::new(registry);

    assert_eq!(
        after_execute_hint("docs.upsert"),
        AfterExecute::EmitDocsUpdated
    );
    assert_eq!(
        after_execute_hint("docs.delete"),
        AfterExecute::EmitDocsUpdated
    );
    assert_eq!(
        after_execute_hint("docs.format"),
        AfterExecute::EmitDocsUpdated
    );
    assert_eq!(
        after_execute_hint("docs.patch"),
        AfterExecute::EmitDocsUpdated
    );

    let created = runner
        .run(
            "docs.upsert",
            "{\"title\":\"bello.today\",\"content\":\"# Hello\\n\\nNext.js readme\"}",
        )
        .unwrap();
    assert!(created.output.contains("created"), "{}", created.output);
    assert!(created.output.contains("bello.today"), "{}", created.output);
    assert!(created.output.contains("html"), "{}", created.output);

    let got = runner
        .run("docs.get", r#"{"id":"bello.today"}"#)
        .unwrap();
    assert!(got.output.contains("Hello"), "{}", got.output);
    assert!(got.output.contains("<h1>"), "{}", got.output);

    let formatted = runner
        .run("docs.format", r#"{"id":"bello.today"}"#)
        .unwrap();
    assert!(formatted.output.contains("formatted"), "{}", formatted.output);

    let patched = runner
        .run(
            "docs.patch",
            r#"{"id":"bello.today","find":"Hello","replace":"Howdy"}"#,
        )
        .unwrap();
    assert!(patched.output.contains("patched"), "{}", patched.output);

    let wiped = runner.run(
        "docs.upsert",
        "{\"title\":\"bello.today\",\"content\":\"\"}",
    );
    assert!(wiped.is_err(), "empty overwrite should fail");

    let updated = runner
        .run(
            "docs.upsert",
            "{\"title\":\"bello.today\",\"content\":\"## Updated\"}",
        )
        .unwrap();
    assert!(updated.output.contains("updated"), "{}", updated.output);

    let deleted = runner
        .run("docs.delete", r#"{"id":"bello.today"}"#)
        .unwrap();
    assert!(deleted.output.contains("deleted"), "{}", deleted.output);

    let err = runner.run("docs.get", r#"{"id":"bello.today"}"#).unwrap_err();
    assert!(err.to_string().contains("not found") || err.to_string().contains("bello.today"));

    let _ = std::fs::remove_dir_all(dir);
}
