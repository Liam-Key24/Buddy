use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use buddy_database::Database;
use buddy_memory::MemoryKind;
use reqwest::Client;
use rusqlite::params;
use tracing::debug;

use super::{
    bytes_to_embedding, cosine_similarity, embed_with_fallback, embedding_to_bytes, final_score,
    recency_score, EmbedRequest, EmbedResponse, IndexRecord, ScoredMemory, SearchError,
    SearchQuery, SemanticSearch,
};

const INDEXABLE_KINDS: [MemoryKind; 8] = [
    MemoryKind::Working,
    MemoryKind::Project,
    MemoryKind::Preference,
    MemoryKind::Handover,
    MemoryKind::Decision,
    MemoryKind::Error,
    MemoryKind::Tool,
    MemoryKind::Reflection,
];

pub struct SqliteEmbeddingBackend {
    db: Arc<Database>,
    brain_url: String,
    client: Client,
}

impl SqliteEmbeddingBackend {
    pub fn new(db: Arc<Database>, brain_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            db,
            brain_url,
            client,
        }
    }

    fn table_name(kind: MemoryKind) -> &'static str {
        kind.table_name()
    }
}

#[async_trait]
impl SemanticSearch for SqliteEmbeddingBackend {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, SearchError> {
        if text.trim().is_empty() {
            return Err(SearchError::Embed("empty text".into()));
        }
        let response = self
            .client
            .post(format!("{}/embed", self.brain_url))
            .json(&EmbedRequest {
                text: text.to_string(),
            })
            .send()
            .await
            .map_err(|e| SearchError::Embed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(SearchError::Embed(format!(
                "brain embed returned {}",
                response.status()
            )));
        }

        let body: EmbedResponse = response
            .json()
            .await
            .map_err(|e| SearchError::Embed(e.to_string()))?;

        if body.embedding.is_empty() {
            return Err(SearchError::InvalidDimensions);
        }
        Ok(body.embedding)
    }

    async fn index(&self, record: IndexRecord) -> Result<(), SearchError> {
        let table = Self::table_name(record.kind);
        let embedding_bytes = embedding_to_bytes(&record.embedding);
        let workspace = record.workspace_path.clone();
        let id = record.id.clone();
        self.db
            .with_conn(|conn| {
                conn.execute(
                    &format!(
                        "UPDATE {table} SET search_text = ?1, embedding = ?2, importance = ?3, updated_at = ?4 WHERE id = ?5 AND workspace_path = ?6"
                    ),
                    params![
                        record.search_text,
                        embedding_bytes,
                        record.importance,
                        record.updated_at,
                        id,
                        workspace,
                    ],
                )?;
                Ok(())
            })
            .map_err(|e| SearchError::Database(e.to_string()))?;
        debug!(kind = ?record.kind, id = %record.id, "indexed memory");
        Ok(())
    }

    async fn search(&self, query: &SearchQuery) -> Result<Vec<ScoredMemory>, SearchError> {
        let now = buddy_database::chrono_now();
        let kinds: Vec<MemoryKind> = query
            .kinds
            .clone()
            .unwrap_or_else(|| INDEXABLE_KINDS.to_vec());

        let mut scored = Vec::new();

        for kind in kinds {
            let table = Self::table_name(kind);
            let workspace = query.workspace_path.clone();
            let rows: Vec<(String, Option<String>, Option<Vec<u8>>, Option<f64>, i64, String)> =
                self.db
                    .with_conn(|conn| {
                        let mut stmt = conn.prepare(&format!(
                            "SELECT id, search_text, embedding, importance, updated_at, payload \
                             FROM {table} WHERE workspace_path = ?1 AND embedding IS NOT NULL"
                        ))?;
                        let rows = stmt.query_map(params![workspace], |row| {
                            Ok((
                                row.get(0)?,
                                row.get(1)?,
                                row.get(2)?,
                                row.get(3)?,
                                row.get(4)?,
                                row.get(5)?,
                            ))
                        })?;
                        rows.collect::<Result<Vec<_>, _>>()
                            .map_err(buddy_database::DbError::from)
                    })
                    .map_err(|e| SearchError::Database(e.to_string()))?;

            for (id, search_text, embedding_blob, importance, updated_at, payload) in rows {
                let Some(blob) = embedding_blob else {
                    continue;
                };
                let embedding = bytes_to_embedding(&blob);
                if embedding.len() != query.query_embedding.len() {
                    continue;
                }
                let similarity = cosine_similarity(&query.query_embedding, &embedding);
                if similarity < query.min_similarity {
                    continue;
                }
                let importance = importance.unwrap_or(0.5);
                let recency = recency_score(updated_at, now);
                let score = final_score(similarity, importance, recency);

                let confidence = serde_json::from_str::<serde_json::Value>(&payload)
                    .ok()
                    .and_then(|p| p.get("confidence").and_then(|v| v.as_f64()));

                if let Some(conf) = confidence {
                    if conf < 0.3 && similarity < 0.85 {
                        continue;
                    }
                }

                scored.push(ScoredMemory {
                    id,
                    kind,
                    search_text: search_text.unwrap_or_default(),
                    payload,
                    similarity: score,
                    importance,
                    updated_at,
                    confidence,
                });
            }
        }

        scored.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(query.limit);
        Ok(scored)
    }

    async fn update(&self, record: IndexRecord) -> Result<(), SearchError> {
        self.index(record).await
    }

    async fn delete(&self, kind: MemoryKind, id: &str) -> Result<(), SearchError> {
        let table = Self::table_name(kind);
        self.db
            .with_conn(|conn| {
                conn.execute(
                    &format!(
                        "UPDATE {table} SET embedding = NULL, search_text = NULL WHERE id = ?1"
                    ),
                    params![id],
                )?;
                Ok(())
            })
            .map_err(|e| SearchError::Database(e.to_string()))?;
        Ok(())
    }
}

impl SqliteEmbeddingBackend {
    pub fn list_unindexed(
        &self,
        workspace_path: &str,
        limit: usize,
    ) -> Result<Vec<(MemoryKind, String, String, f64, i64)>, SearchError> {
        let mut results = Vec::new();
        for kind in INDEXABLE_KINDS {
            let table = Self::table_name(kind);
            let workspace = workspace_path.to_string();
            let rows: Vec<(String, String, f64, i64)> = self
                .db
                .with_conn(|conn| {
                    let mut stmt = conn.prepare(&format!(
                        "SELECT id, payload, COALESCE(importance, 0.5), updated_at \
                         FROM {table} WHERE workspace_path = ?1 AND embedding IS NULL LIMIT ?2"
                    ))?;
                    let rows = stmt.query_map(params![workspace, limit as i64], |row| {
                        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                    })?;
                    rows.collect::<Result<Vec<_>, _>>()
                        .map_err(buddy_database::DbError::from)
                })
                .map_err(|e| SearchError::Database(e.to_string()))?;
            for (id, payload, importance, updated_at) in rows {
                results.push((kind, id, payload, importance, updated_at));
            }
        }
        Ok(results)
    }

    pub fn db(&self) -> Arc<Database> {
        self.db.clone()
    }

    pub async fn embed_with_fallback(&self, text: &str) -> Vec<f32> {
        embed_with_fallback(self, text).await
    }
}
