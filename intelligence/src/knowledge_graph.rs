use std::sync::Arc;

use buddy_database::Database;
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::semantic::ScoredMemory;

#[derive(Debug, Clone)]
pub struct KgEntity {
    pub id: String,
    pub name: String,
    pub entity_type: String,
}

pub struct KnowledgeGraph {
    db: Arc<Database>,
}

impl KnowledgeGraph {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn upsert_entity(
        &self,
        workspace_path: &str,
        name: &str,
        entity_type: &str,
        source_kind: Option<&str>,
        source_id: Option<&str>,
    ) -> Result<String, String> {
        let now = buddy_database::chrono_now();
        let existing: Option<String> = self
            .db
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT id FROM kg_entities WHERE workspace_path = ?1 AND name = ?2 AND entity_type = ?3",
                    params![workspace_path, name, entity_type],
                    |row| row.get(0),
                )
                .optional()
                .map_err(buddy_database::DbError::from)
            })
            .map_err(|e| e.to_string())?;

        if let Some(id) = existing {
            self.db
                .with_conn(|conn| {
                    conn.execute(
                        "UPDATE kg_entities SET updated_at = ?1 WHERE id = ?2",
                        params![now, id],
                    )?;
                    Ok(())
                })
                .map_err(|e| e.to_string())?;
            return Ok(id);
        }

        let id = Uuid::new_v4().to_string();
        self.db
            .with_conn(|conn| {
                conn.execute(
                    "INSERT INTO kg_entities (id, workspace_path, name, entity_type, source_kind, source_id, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        id,
                        workspace_path,
                        name,
                        entity_type,
                        source_kind,
                        source_id,
                        now,
                        now,
                    ],
                )?;
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        Ok(id)
    }

    pub fn add_relation(
        &self,
        workspace_path: &str,
        from_id: &str,
        to_id: &str,
        relation_type: &str,
        weight: f64,
    ) -> Result<(), String> {
        let id = Uuid::new_v4().to_string();
        let now = buddy_database::chrono_now();
        self.db
            .with_conn(|conn| {
                conn.execute(
                    "INSERT INTO kg_relations (id, workspace_path, from_entity_id, to_entity_id, relation_type, weight, created_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![id, workspace_path, from_id, to_id, relation_type, weight, now],
                )?;
                Ok(())
            })
            .map_err(|e| e.to_string())
    }

    pub fn ingest_decision(
        &self,
        workspace_path: &str,
        decision: &str,
        reason: &str,
        source_id: Option<&str>,
    ) -> Result<(), String> {
        let decision_id = self.upsert_entity(
            workspace_path,
            decision,
            "decision",
            Some("decision"),
            source_id,
        )?;

        for tech in extract_technologies(&format!("{decision} {reason}")) {
            let tech_id = self.upsert_entity(workspace_path, &tech, "technology", None, None)?;
            let _ = self.add_relation(workspace_path, &decision_id, &tech_id, "uses", 1.0);
        }
        Ok(())
    }

    pub fn ingest_preference(
        &self,
        workspace_path: &str,
        key: &str,
        value: &str,
    ) -> Result<(), String> {
        let pref_id = self.upsert_entity(workspace_path, key, "preference", Some("preference"), None)?;
        let val_id = self.upsert_entity(workspace_path, value, "technology", None, None)?;
        let _ = self.add_relation(workspace_path, &pref_id, &val_id, "prefers", 1.0);
        Ok(())
    }

    pub fn ingest_project_section(
        &self,
        workspace_path: &str,
        section: &str,
        content: &str,
    ) -> Result<(), String> {
        let section_id = self.upsert_entity(workspace_path, section, "project", Some("project"), None)?;
        for tech in extract_technologies(content) {
            let tech_id = self.upsert_entity(workspace_path, &tech, "technology", None, None)?;
            let _ = self.add_relation(workspace_path, &section_id, &tech_id, "uses", 1.0);
        }
        Ok(())
    }

    pub fn ingest_llm_entities(
        &self,
        workspace_path: &str,
        entities: &[serde_json::Value],
        relations: &[serde_json::Value],
    ) -> Result<(), String> {
        let mut id_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        for entity in entities {
            let name = entity.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let entity_type = entity
                .get("entity_type")
                .and_then(|v| v.as_str())
                .unwrap_or("concept");
            if name.is_empty() {
                continue;
            }
            let id = self.upsert_entity(workspace_path, name, entity_type, None, None)?;
            id_map.insert(name.to_string(), id);
        }

        for relation in relations {
            let from = relation.get("from").and_then(|v| v.as_str()).unwrap_or("");
            let to = relation.get("to").and_then(|v| v.as_str()).unwrap_or("");
            let rel_type = relation
                .get("relation_type")
                .and_then(|v| v.as_str())
                .unwrap_or("related_to");
            if from.is_empty() || to.is_empty() {
                continue;
            }
            let from_id = id_map.get(from).cloned().unwrap_or_else(|| {
                self.upsert_entity(workspace_path, from, "concept", None, None)
                    .unwrap_or_default()
            });
            let to_id = id_map.get(to).cloned().unwrap_or_else(|| {
                self.upsert_entity(workspace_path, to, "concept", None, None)
                    .unwrap_or_default()
            });
            let _ = self.add_relation(workspace_path, &from_id, &to_id, rel_type, 1.0);
        }
        Ok(())
    }

    pub fn related_context(
        &self,
        workspace_path: &str,
        hits: &[ScoredMemory],
    ) -> Result<String, String> {
        let mut lines = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for hit in hits.iter().take(5) {
            let entities: Vec<KgEntity> = self
                .db
                .with_conn(|conn| {
                    let mut stmt = conn.prepare(
                        "SELECT id, name, entity_type FROM kg_entities \
                         WHERE workspace_path = ?1 AND source_id = ?2 LIMIT 5",
                    )?;
                    let rows = stmt.query_map(params![workspace_path, hit.id], |row| {
                        Ok(KgEntity {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            entity_type: row.get(2)?,
                        })
                    })?;
                    rows.collect::<Result<Vec<_>, _>>()
                        .map_err(buddy_database::DbError::from)
                })
                .map_err(|e| e.to_string())?;

            for entity in entities {
                let related: Vec<String> = self
                    .db
                    .with_conn(|conn| {
                        let mut stmt = conn.prepare(
                            "SELECT e.name, e.entity_type, r.relation_type \
                             FROM kg_relations r \
                             JOIN kg_entities e ON e.id = r.to_entity_id \
                             WHERE r.workspace_path = ?1 AND r.from_entity_id = ?2 LIMIT 5",
                        )?;
                        let rows = stmt.query_map(params![workspace_path, entity.id], |row| {
                            let name: String = row.get(0)?;
                            let etype: String = row.get(1)?;
                            let rel: String = row.get(2)?;
                            Ok(format!("{name} ({etype}, {rel})"))
                        })?;
                        rows.collect::<Result<Vec<_>, _>>()
                            .map_err(buddy_database::DbError::from)
                    })
                    .map_err(|e| e.to_string())?;

                for r in related {
                    if seen.insert(r.clone()) {
                        lines.push(format!("- {r}"));
                    }
                }
            }
        }

        Ok(lines.join("\n"))
    }
}

fn extract_technologies(text: &str) -> Vec<String> {
    const TECHS: &[&str] = &[
        "Rust", "Python", "React", "TypeScript", "SQLite", "Tauri", "MLX", "FastAPI",
        "JavaScript", "Go", "Node.js", "local-first", "local first",
    ];
    let lower = text.to_lowercase();
    TECHS.iter()
        .filter(|t| lower.contains(&t.to_lowercase()))
        .map(|t| (*t).to_string())
        .collect()
}