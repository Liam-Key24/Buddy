use buddy_memory::MemoryKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexRecord {
    pub id: String,
    pub kind: MemoryKind,
    pub workspace_path: String,
    pub search_text: String,
    pub embedding: Vec<f32>,
    pub importance: f64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub workspace_path: String,
    pub query_embedding: Vec<f32>,
    pub kinds: Option<Vec<MemoryKind>>,
    pub limit: usize,
    pub min_similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredMemory {
    pub id: String,
    pub kind: MemoryKind,
    pub search_text: String,
    pub payload: String,
    pub similarity: f32,
    pub importance: f64,
    pub updated_at: i64,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedRequest {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedResponse {
    pub embedding: Vec<f32>,
    pub dimensions: usize,
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

pub fn embedding_to_bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect()
}

pub fn bytes_to_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

pub fn recency_score(updated_at: i64, now: i64) -> f32 {
    let age_secs = (now - updated_at).max(0) as f32;
    let age_days = age_secs / 86400.0;
    (-age_days / 30.0).exp()
}

pub fn final_score(similarity: f32, importance: f64, recency: f32) -> f32 {
    similarity * 0.6 + (importance as f32) * 0.25 + recency * 0.15
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_identical_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn embedding_roundtrip() {
        let original = vec![0.1, -0.2, 0.3];
        let bytes = embedding_to_bytes(&original);
        let restored = bytes_to_embedding(&bytes);
        assert_eq!(original, restored);
    }
}
