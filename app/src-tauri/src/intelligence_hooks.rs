use buddy_memory::{MemoryContext, SavedMemory};
use tracing::warn;

use crate::state::AppState;

pub fn spawn_index_saved(state: &AppState, ctx: &MemoryContext, saved: &[SavedMemory]) {
    for item in saved {
        let intelligence = state.intelligence.clone();
        let ctx = ctx.clone();
        let kind = item.kind;
        let id = item.id.clone();
        let payload = item.payload.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = intelligence
                .on_memory_saved(&ctx, kind, &id, &payload)
                .await
            {
                warn!(error = %e, kind = ?kind, "memory indexing failed");
            }
        });
    }
}

pub async fn index_saved_sync(
    state: &AppState,
    ctx: &MemoryContext,
    saved: &[SavedMemory],
) {
    for item in saved {
        if let Err(e) = state
            .intelligence
            .on_memory_saved(&ctx, item.kind, &item.id, &item.payload)
            .await
        {
            warn!(error = %e, kind = ?item.kind, "memory indexing failed");
        }
    }
}
