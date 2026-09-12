//! Control plane for a chat turn.
//!
//! Every user message enters [`TurnController::handle`]. Deterministic gates
//! run first; model adaptation is a hidden [`ModelLane`] policy. Llama is never
//! a product mode. `/chat/plan` and `brain/parser.py` stay eval-only.

use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::native_loop::ChatMode;
use crate::orchestrator;
use crate::run_control::RunScope;
use crate::runtime_policy::RuntimePolicy;
use crate::state::AppState;
use crate::turn_trace::{self, TurnPath, TurnTrace};

/// Explicit Cool Mode turn states. The controller owns the transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnPhase {
    Understand,
    Clarify,
    Propose,
    Approve,
    Execute,
    Present,
}

impl TurnPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Understand => "understand",
            Self::Clarify => "clarify",
            Self::Propose => "propose",
            Self::Approve => "approve",
            Self::Execute => "execute",
            Self::Present => "present",
        }
    }
}

/// One live turn: policy, clock, and the trace the gates mutate.
pub struct TurnSession {
    pub policy: RuntimePolicy,
    pub started: Instant,
    pub trace: TurnTrace,
}

impl TurnSession {
    pub fn begin(conversation_id: &str, text: &str, ui_context: Option<&str>) -> Self {
        let mut trace = TurnTrace::new(conversation_id);
        trace.skill_ids = crate::skills::skill_ids_for_turn(text, ui_context)
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let mut session = Self {
            policy: RuntimePolicy::cool(),
            started: Instant::now(),
            trace,
        };
        session.set_phase(TurnPhase::Understand);
        session
    }

    pub fn set_phase(&mut self, phase: TurnPhase) {
        self.trace.phase = Some(phase.as_str().into());
    }
}

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
    /// Only production front door for chat. Rejects a second live turn.
    pub async fn handle(
        app: AppHandle,
        state: &AppState,
        request: TurnRequest,
    ) -> Result<(), String> {
        let mut session = TurnSession::begin(
            &request.conversation_id,
            &request.text,
            request.ui_context.as_deref(),
        );

        let scope = match RunScope::try_start(&state.runs, &request.conversation_id) {
            Ok(scope) => scope,
            Err(busy) => {
                let content = "I'm still finishing the last request. Stop it first if you want to start something new.";
                let _ = app.emit("chat-chunk", content);
                session.trace.exit_reason = Some("busy".into());
                session.set_phase(TurnPhase::Present);
                let ctx = state.memory.ctx(&request.conversation_id);
                session.trace.set_path(TurnPath::Busy);
                turn_trace::finish_turn_trace(&app, state, &mut session.trace, session.started);
                orchestrator::persist_assistant_turn(
                    &app,
                    state,
                    &ctx,
                    &request.conversation_id,
                    content,
                    &json!({"intent":"busy","active": busy.conversation_id}).to_string(),
                )?;
                return Ok(());
            }
        };

        orchestrator::run_turn(
            app,
            state,
            request.conversation_id,
            request.text,
            request.ui_context,
            &scope.guard,
            &mut session,
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
        let session = TurnSession::begin(&conversation_id, &value, None);
        let scope = match RunScope::try_start(&state.runs, &conversation_id) {
            Ok(scope) => scope,
            Err(_) => {
                return Err(
                    "I'm still finishing the last request. Stop it first if you want to continue."
                        .into(),
                );
            }
        };
        orchestrator::resolve_clarification(
            app,
            state,
            conversation_id,
            field,
            value,
            &scope.guard,
            &session.policy,
        )
        .await
    }

    /// Only place the production turn picks a hidden inference lane.
    pub fn lane_for(text: &str, ui_context: Option<&str>, qwen_resident: bool) -> ModelLane {
        ModelLane::select(crate::native_loop::is_trivial_chat(text, ui_context), qwen_resident)
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

    #[test]
    fn llama_never_maps_to_a_tool_lane() {
        assert_ne!(ModelLane::LlamaTalk.native_mode(), ChatMode::Tool);
        assert_eq!(ModelLane::LlamaTalk.path(), TurnPath::TalkLlama);
        assert_eq!(ModelLane::QwenComplete.path(), TurnPath::CompleteQwen);
    }

    #[test]
    fn controller_picks_the_lane_from_text() {
        assert_eq!(
            TurnController::lane_for("hey", None, false),
            ModelLane::LlamaTalk
        );
        assert_eq!(
            TurnController::lane_for("book the dentist tomorrow at 2", None, false),
            ModelLane::QwenComplete
        );
    }

    #[test]
    fn session_starts_in_understand() {
        let session = TurnSession::begin("c1", "hello", None);
        assert_eq!(session.trace.phase.as_deref(), Some("understand"));
        assert_eq!(TurnPhase::Present.as_str(), "present");
    }
}
