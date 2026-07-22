//! Future sync provider surface. Not registered or implemented in v1.
//! Google, Outlook, iCloud, CalDAV, etc. can implement [`SyncProvider`]
//! without changing core calendar CRUD or UI contracts.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::models::{DateRange, Event};

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("{0}")]
    Message(String),
    #[error("not configured")]
    NotConfigured,
    #[error("auth expired")]
    AuthExpired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalEvent {
    pub external_id: String,
    pub event: Event,
}

/// Optional bidirectional sync adapter for external calendar providers.
#[async_trait]
pub trait SyncProvider: Send + Sync {
    fn id(&self) -> &'static str;
    async fn pull(&self, range: DateRange) -> Result<Vec<ExternalEvent>, SyncError>;
    async fn push(&self, event: &Event) -> Result<(), SyncError>;
}
