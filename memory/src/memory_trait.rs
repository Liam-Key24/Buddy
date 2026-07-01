use crate::error::MemoryError;
use crate::types::{MemoryContext, MemoryKind, MemoryRecord, RetrieveQuery};

pub trait Memory: Send + Sync {
    fn kind(&self) -> MemoryKind;
    fn save(&self, ctx: &MemoryContext, record: MemoryRecord) -> Result<String, MemoryError>;
    fn retrieve(&self, query: &RetrieveQuery) -> Result<Vec<MemoryRecord>, MemoryError>;
    fn update(&self, id: &str, record: MemoryRecord) -> Result<(), MemoryError>;
    fn delete(&self, id: &str) -> Result<(), MemoryError>;
    fn summarize(&self, query: &RetrieveQuery) -> Result<String, MemoryError>;
}
