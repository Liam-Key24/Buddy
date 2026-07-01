use std::sync::Arc;

use buddy_database::Database;
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

pub struct LearningEngine {
    db: Arc<Database>,
}

impl LearningEngine {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn observe_preference(
        &self,
        workspace_path: &str,
        key: &str,
        value: &str,
    ) -> Result<(), String> {
        let pattern_type = classify_preference_key(key);
        let description = format!("Prefers {key}: {value}");
        self.upsert_pattern(workspace_path, pattern_type, &description, 0.7)
    }

    pub fn observe_tool(&self, workspace_path: &str, tool: &str) -> Result<(), String> {
        let description = format!("Frequently uses tool: {tool}");
        self.upsert_pattern(workspace_path, "tool", &description, 0.6)
    }

    pub fn observe_decision(&self, workspace_path: &str, decision: &str) -> Result<(), String> {
        let description = format!("Architecture pattern: {decision}");
        self.upsert_pattern(workspace_path, "architecture", &description, 0.75)
    }

    pub fn on_task_complete(&self, workspace_path: &str, outcome: &str) -> Result<(), String> {
        if outcome.len() > 20 {
            let description = format!("Task pattern: {}", outcome.chars().take(100).collect::<String>());
            self.upsert_pattern(workspace_path, "workflow", &description, 0.5)
        } else {
            Ok(())
        }
    }

    fn upsert_pattern(
        &self,
        workspace_path: &str,
        pattern_type: &str,
        description: &str,
        base_confidence: f64,
    ) -> Result<(), String> {
        let now = buddy_database::chrono_now();
        let existing: Option<(String, f64, i64)> = self
            .db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT id, confidence, observation_count FROM learned_patterns \
                     WHERE workspace_path = ?1 AND pattern_type = ?2 AND description = ?3",
                    params![workspace_path, pattern_type, description],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()
                .map_err(buddy_database::DbError::from)
            })
            .map_err(|e| e.to_string())?;

        if let Some((id, confidence, count)) = existing {
            let new_confidence = (confidence + 0.1).min(0.99);
            let new_count = count + 1;
            self.db
                .with_conn(|conn| {
                    conn.execute(
                        "UPDATE learned_patterns SET confidence = ?1, observation_count = ?2, last_confirmed_at = ?3 \
                         WHERE id = ?4",
                        params![new_confidence, new_count, now, id],
                    )?;
                    Ok(())
                })
                .map_err(|e| e.to_string())?;
        } else {
            let id = Uuid::new_v4().to_string();
            self.db
                .with_conn(|conn| {
                    conn.execute(
                        "INSERT INTO learned_patterns \
                         (id, workspace_path, pattern_type, description, evidence_json, confidence, observation_count, last_confirmed_at, created_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8)",
                        params![
                            id,
                            workspace_path,
                            pattern_type,
                            description,
                            "{}",
                            base_confidence,
                            now,
                            now,
                        ],
                    )?;
                    Ok(())
                })
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn decay_contradicted(
        &self,
        workspace_path: &str,
        pattern_type: &str,
        description: &str,
    ) -> Result<(), String> {
        self.db
            .with_conn(|conn| {
                conn.execute(
                    "UPDATE learned_patterns SET confidence = confidence * 0.5 \
                     WHERE workspace_path = ?1 AND pattern_type = ?2 AND description != ?3",
                    params![workspace_path, pattern_type, description],
                )?;
                Ok(())
            })
            .map_err(|e| e.to_string())
    }

    pub fn format_for_context(&self, workspace_path: &str) -> Result<String, String> {
        let patterns: Vec<(String, f64)> = self
            .db
            .with_conn(|conn| {
                let mut stmt = conn.prepare(
                    "SELECT description, confidence FROM learned_patterns \
                     WHERE workspace_path = ?1 AND confidence >= 0.7 ORDER BY confidence DESC LIMIT 10",
                )?;
                let rows = stmt.query_map(params![workspace_path], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(buddy_database::DbError::from)
            })
            .map_err(|e| e.to_string())?;

        if patterns.is_empty() {
            return Ok(String::new());
        }

        let lines: Vec<String> = patterns
            .iter()
            .map(|(desc, conf)| format!("- {desc} (confidence: {:.0}%)", conf * 100.0))
            .collect();
        Ok(format!("Learned patterns:\n{}", lines.join("\n")))
    }
}

fn classify_preference_key(key: &str) -> &'static str {
    let lower = key.to_lowercase();
    if lower.contains("language") || lower.contains("lang") {
        "language"
    } else if lower.contains("arch") {
        "architecture"
    } else if lower.contains("tool") {
        "tool"
    } else {
        "workflow"
    }
}