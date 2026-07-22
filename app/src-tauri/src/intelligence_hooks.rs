use buddy_memory::{MemoryContext, SavedMemory};

use crate::state::AppState;

pub async fn index_saved_sync(state: &AppState, ctx: &MemoryContext, saved: &[SavedMemory]) {
    state.memory.index_saved_sync(ctx, saved).await;
}
