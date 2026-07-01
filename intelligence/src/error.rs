use thiserror::Error;

#[derive(Debug, Error)]
pub enum IntelligenceError {
    #[error("memory error: {0}")]
    Memory(#[from] buddy_memory::MemoryError),
    #[error("search error: {0}")]
    Search(#[from] crate::semantic::SearchError),
    #[error("database error: {0}")]
    Database(String),
    #[error("{0}")]
    Other(String),
}
