mod sqlite;
mod types;

pub use sqlite::SqliteEmbeddingBackend;
pub use types::*;

use async_trait::async_trait;
use thiserror::Error;
use tracing::warn;

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("embed failed: {0}")]
    Embed(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("invalid embedding dimensions")]
    InvalidDimensions,
}

#[async_trait]
pub trait SemanticSearch: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, SearchError>;
    async fn index(&self, record: IndexRecord) -> Result<(), SearchError>;
    async fn search(&self, query: &SearchQuery) -> Result<Vec<ScoredMemory>, SearchError>;
    async fn update(&self, record: IndexRecord) -> Result<(), SearchError>;
    async fn delete(&self, kind: MemoryKind, id: &str) -> Result<(), SearchError>;
}

use buddy_memory::MemoryKind;

pub fn hash_fallback_embedding(text: &str) -> Vec<f32> {
    const DIM: usize = 384;
    let mut vec = vec![0.0f32; DIM];
    for (i, byte) in text.bytes().enumerate() {
        vec[i % DIM] += (byte as f32) / 255.0;
    }
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut vec {
            *v /= norm;
        }
    }
    vec
}

pub async fn embed_with_fallback<S: SemanticSearch + ?Sized>(
    search: &S,
    text: &str,
) -> Vec<f32> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    match search.embed(text).await {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "embed failed, using hash fallback");
            hash_fallback_embedding(text)
        }
    }
}
