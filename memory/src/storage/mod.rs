mod sqlite;

pub use sqlite::SqliteStorageBackend;

use crate::error::StorageError;
use crate::types::MemoryKind;

#[derive(Debug, Clone)]
pub struct MemoryRow {
    pub id: String,
    pub workspace_path: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub payload: String,
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
