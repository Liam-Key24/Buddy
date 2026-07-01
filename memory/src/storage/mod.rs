mod sqlite;

pub use sqlite::SqliteStorageBackend;

pub fn default_importance(kind: crate::types::MemoryKind) -> f64 {
    sqlite::default_importance(kind)
}

use crate::error::StorageError;
use crate::types::MemoryKind;

#[derive(Debug, Clone, Default)]
pub struct MemoryRow {
    pub id: String,
    pub workspace_path: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub payload: String,
    pub search_text: Option<String>,
    pub embedding: Option<Vec<u8>>,
    pub importance: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct StorageQuery {
    pub table: MemoryKind,
    pub workspace_path: String,
    pub limit: Option<usize>,
    pub order_desc: bool,
}

pub trait StorageBackend: Send + Sync {
    fn insert(&self, table: MemoryKind, row: &MemoryRow) -> Result<String, StorageError>;
    fn query(&self, query: StorageQuery) -> Result<Vec<MemoryRow>, StorageError>;
    fn update(&self, table: MemoryKind, id: &str, row: &MemoryRow) -> Result<(), StorageError>;
    fn delete(&self, table: MemoryKind, id: &str) -> Result<(), StorageError>;
    fn delete_by_workspace(&self, table: MemoryKind, workspace_path: &str) -> Result<(), StorageError>;
}
