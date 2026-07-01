use std::sync::Arc;

use buddy_database::Database;
use buddy_memory::MemoryManager;
use rusqlite::{params, OptionalExtension};
use serde_json::json;

use crate::knowledge_graph::KnowledgeGraph;
use crate::learning::LearningEngine;

#[derive(Debug, Clone, Default)]
pub struct WorkspaceProfile {
    pub name: Option<String>,
    pub goals: Option<String>,
    pub current_milestone: Option<String>,
    pub stack: Vec<String>,
    pub architecture: serde_json::Value,
    pub features: Vec<String>,
    pub active_tasks: Vec<String>,
    pub recent_decisions: Vec<String>,
    pub known_issues: Vec<String>,
}

pub struct WorkspaceIntel {
    db: Arc<Database>,
}

impl WorkspaceIntel {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn summary(&self, workspace_path: &str) -> Result<String, String> {
        let profile = self.load(workspace_path)?;
        let mut parts = Vec::new();

        if let Some(name) = &profile.name {
            parts.push(format!("Name: {name}"));
        }
        if let Some(milestone) = &profile.current_milestone {
            parts.push(format!("Current milestone: {milestone}"));
        }
        if let Some(goals) = &profile.goals {
            parts.push(format!("Goals: {goals}"));
        }
        if !profile.stack.is_empty() {
            parts.push(format!("Technology: {}", profile.stack.join(", ")));
        }
        if !profile.recent_decisions.is_empty() {
            parts.push(format!(
                "Recent decisions: {}",
                profile.recent_decisions.join("; ")
            ));
        }
        if !profile.active_tasks.is_empty() {
            parts.push(format!(
                "Active tasks: {}",
                profile.active_tasks.join(", ")
            ));
        }

        Ok(parts.join("\n"))
    }

    pub fn load(&self, workspace_path: &str) -> Result<WorkspaceProfile, String> {
        let row: Option<(
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = self.db
                .with_conn(|conn| {
                    conn.query_row(
                        "SELECT name, goals, current_milestone, stack_json, architecture_json, \
                         features_json, active_tasks_json, recent_decisions_json, known_issues_json \
                         FROM workspace_profiles WHERE workspace_path = ?1",
                        params![workspace_path],
                        |row| {
                            Ok((
                                row.get(0)?,
                                row.get(1)?,
                                row.get(2)?,
                                row.get(3)?,
                                row.get(4)?,
                                row.get(5)?,
                                row.get(6)?,
                                row.get(7)?,
                                row.get(8)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(buddy_database::DbError::from)
                })
                .map_err(|e| e.to_string())?;

        if let Some((
            name,
            goals,
            milestone,
            stack_json,
            arch_json,
            features_json,
            tasks_json,
            decisions_json,
            issues_json,
        )) = row
        {
            Ok(WorkspaceProfile {
                name,
                goals,
                current_milestone: milestone,
                stack: parse_json_array(stack_json.as_deref().unwrap_or("[]")),
                architecture: serde_json::from_str(arch_json.as_deref().unwrap_or("{}")).unwrap_or(json!({})),
                features: parse_json_array(features_json.as_deref().unwrap_or("[]")),
                active_tasks: parse_json_array(tasks_json.as_deref().unwrap_or("[]")),
                recent_decisions: parse_json_array(decisions_json.as_deref().unwrap_or("[]")),
                known_issues: parse_json_array(issues_json.as_deref().unwrap_or("[]")),
            })
        } else {
            Ok(WorkspaceProfile::default())
        }
    }

    pub fn refresh(
        &self,
        workspace_path: &str,
        memory: &MemoryManager,
        kg: &KnowledgeGraph,
        learning: &LearningEngine,
    ) -> Result<(), String> {
        use buddy_memory::{MemoryContext, MemoryKind, RetrieveQuery};

        let ctx = MemoryContext {
            workspace_path: workspace_path.into(),
            conversation_id: None,
            task_id: None,
        };

        let query = RetrieveQuery {
            workspace_path: workspace_path.into(),
            conversation_id: None,
            task_id: None,
            keywords: None,
            limit: Some(20),
        };

        let name = std::path::Path::new(workspace_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Workspace")
            .to_string();

        let project = memory.summarize_kind(MemoryKind::Project, &query).unwrap_or_default();
        let decisions = memory.summarize_kind(MemoryKind::Decision, &query).unwrap_or_default();
        let working = memory.summarize_kind(MemoryKind::Working, &query).unwrap_or_default();
        let errors = memory.summarize_kind(MemoryKind::Error, &query).unwrap_or_default();

        let stack = extract_stack_from_text(&format!("{project} {decisions}"));
        let recent_decisions: Vec<String> = decisions
            .lines()
            .filter(|l| l.starts_with("- "))
            .map(|l| l.trim_start_matches("- ").to_string())
            .take(5)
            .collect();

        let active_tasks: Vec<String> = working
            .lines()
            .filter(|l| !l.is_empty())
            .map(String::from)
            .take(5)
            .collect();

        let known_issues: Vec<String> = errors
            .lines()
            .filter(|l| l.starts_with("- "))
            .map(|l| l.trim_start_matches("- ").to_string())
            .take(5)
            .collect();

        let milestone = if project.contains("Intelligence") {
            Some("Intelligence Layer".to_string())
        } else if !project.is_empty() {
            Some("Active development".to_string())
        } else {
            None
        };

        let _ = learning;
        let _ = kg;

        let now = buddy_database::chrono_now();
        self.db
            .with_conn(|conn| {
                conn.execute(
                    "INSERT INTO workspace_profiles \
                     (workspace_path, name, goals, current_milestone, stack_json, architecture_json, \
                      features_json, active_tasks_json, recent_decisions_json, known_issues_json, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
                     ON CONFLICT(workspace_path) DO UPDATE SET \
                     name = excluded.name, goals = excluded.goals, current_milestone = excluded.current_milestone, \
                     stack_json = excluded.stack_json, architecture_json = excluded.architecture_json, \
                     features_json = excluded.features_json, active_tasks_json = excluded.active_tasks_json, \
                     recent_decisions_json = excluded.recent_decisions_json, known_issues_json = excluded.known_issues_json, \
                     updated_at = excluded.updated_at",
                    params![
                        workspace_path,
                        name,
                        project.chars().take(200).collect::<String>(),
                        milestone,
                        serde_json::to_string(&stack).unwrap_or_else(|_| "[]".into()),
                        json!({"Core": ["Rust"], "Brain": ["Python", "MLX"], "UI": ["React", "Tauri"]}).to_string(),
                        "[]",
                        serde_json::to_string(&active_tasks).unwrap_or_else(|_| "[]".into()),
                        serde_json::to_string(&recent_decisions).unwrap_or_else(|_| "[]".into()),
                        serde_json::to_string(&known_issues).unwrap_or_else(|_| "[]".into()),
                        now,
                    ],
                )?;
                Ok(())
            })
            .map_err(|e| e.to_string())?;

        let _ = ctx;
        Ok(())
    }
}

fn parse_json_array(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}

fn extract_stack_from_text(text: &str) -> Vec<String> {
    const TECHS: &[&str] = &[
        "Rust", "Python", "React", "TypeScript", "SQLite", "Tauri", "MLX", "FastAPI",
    ];
    let lower = text.to_lowercase();
    TECHS.iter()
        .filter(|t| lower.contains(&t.to_lowercase()))
        .map(|t| (*t).to_string())
        .collect()
}