use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("database error: {0}")]
    Database(#[from] buddy_database::DbError),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("memory not found: {0}")]
    NotFound(String),
    #[error("invalid memory: {0}")]
    Invalid(String),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] buddy_database::DbError),
    #[error("unknown table: {0}")]
    UnknownTable(String),
}
