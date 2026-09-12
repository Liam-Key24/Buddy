//! Single control plane for a chat turn.
//!
//! Every user message enters [`TurnController::handle`]. Deterministic gates
//! run first; model adaptation is a hidden [`ModelLane`] policy. Llama is never
//! a product mode. `/chat/plan` and `brain/parser.py` stay eval-only.

use serde::{Deserialize, Serialize};

use crate::native_loop::ChatMode;
use crate::orchestrator;
use crate::state::AppState;
use crate::turn_trace::TurnPath;
use tauri::AppHandle;

/// Envelope for one user turn. UI does not choose a model.
#[derive(Debug, Clone)]
pub struct TurnRequest {
    pub conversation_id: String,
    pub text: String,
    pub ui_context: Option<String>,
}

/// Hidden inference lane. Not a user-facing ChatMode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelLane {
    /// Short chitchat while Qwen is cold — Llama `/chat/talk`.
    LlamaTalk,
    /// Default production lane — Qwen `/v1/complete`.
    QwenComplete,
}

impl ModelLane {
    pub fn select(trivial_chat: bool, qwen_resident: bool) -> Self {
        if trivial_chat && !qwen_resident {
            Self::LlamaTalk
        } else {
            Self::QwenComplete
        }
    }

    pub fn native_mode(self) -> ChatMode {
        match self {
            Self::LlamaTalk => ChatMode::Talk,
            Self::QwenComplete => ChatMode::Tool,
        }
    }

    pub fn path(self) -> TurnPath {
        match self {
            Self::LlamaTalk => TurnPath::TalkLlama,
            Self::QwenComplete => TurnPath::CompleteQwen,
        }
    }
}

pub struct TurnController;

impl TurnController {
    /// Only production front door for chat.
    pub async fn handle(
        app: AppHandle,
        state: &AppState,
        request: TurnRequest,
    ) -> Result<(), String> {
        orchestrator::send_message(
            app,
            state,
            request.conversation_id,
            request.text,
            request.ui_context,
        )
        .await
    }

    pub async fn resolve_clarification(
        app: AppHandle,
        state: &AppState,
        conversation_id: String,
        field: String,
        value: String,
    ) -> Result<(), String> {
        orchestrator::resolve_clarification(app, state, conversation_id, field, value).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llama_is_a_cold_qwen_cache_not_a_product_mode() {
        assert_eq!(ModelLane::select(true, false), ModelLane::LlamaTalk);
        assert_eq!(ModelLane::select(true, true), ModelLane::QwenComplete);
        assert_eq!(ModelLane::select(false, false), ModelLane::QwenComplete);
        assert_eq!(ModelLane::select(false, true), ModelLane::QwenComplete);
    }

    #[test]
    fn llama_lane_maps_to_internal_talk_flag_only() {
        assert_eq!(ModelLane::LlamaTalk.native_mode(), ChatMode::Talk);
        assert_eq!(ModelLane::QwenComplete.native_mode(), ChatMode::Tool);
    }
}
