//! Last-turn route trace: which path ran, which model, and how much it cost.

use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnPath {
    ForceConfirm,
    ProposalCommit,
    ProposalCancel,
    Canonical,
    Extract,
    LastLook,
    ResumeNative,
    OpenJob,
    TalkLlama,
    TalkRespond,
    CompleteQwen,
    ModelError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnTrace {
    pub conversation_id: String,
    pub path: Option<TurnPath>,
    pub model: Option<String>,
    pub model_call_count: u32,
    pub attached_tool_count: u32,
    pub tool_steps: u32,
    pub clarification_count: u32,
    pub latency_ms: u64,
}

impl TurnTrace {
    pub fn new(conversation_id: impl Into<String>) -> Self {
        Self {
            conversation_id: conversation_id.into(),
            path: None,
            model: None,
            model_call_count: 0,
            attached_tool_count: 0,
            tool_steps: 0,
            clarification_count: 0,
            latency_ms: 0,
        }
    }

    pub fn set_path(&mut self, path: TurnPath) {
        if self.path.is_none() {
            self.path = Some(path);
        }
    }
}

/// Deterministic / resume paths must not spend a model call.
pub fn path_expects_zero_model_calls(path: TurnPath) -> bool {
    matches!(
        path,
        TurnPath::ForceConfirm
            | TurnPath::ProposalCommit
            | TurnPath::ProposalCancel
            | TurnPath::Canonical
            | TurnPath::Extract
            | TurnPath::LastLook
            | TurnPath::OpenJob
    )
}

/// Hidden Llama cache only when the turn is short chitchat and Qwen is not resident.
pub fn talk_policy_path(trivial: bool, qwen_resident: bool) -> TurnPath {
    if trivial && !qwen_resident {
        TurnPath::TalkLlama
    } else {
        TurnPath::CompleteQwen
    }
}

pub fn finish_turn_trace(
    app: &AppHandle,
    state: &AppState,
    trace: &mut TurnTrace,
    started: Instant,
) {
    trace.latency_ms = started.elapsed().as_millis() as u64;
    if let Ok(raw) = serde_json::to_string(trace) {
        state.memory.set_turn_trace(&trace.conversation_id, &raw);
    }
    let path = trace
        .path
        .map(|p| {
            serde_json::to_value(p)
                .ok()
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default()
        })
        .unwrap_or_default();
    let detail = format!(
        "path={path} model={} calls={} tools={} steps={} clarify={} {}ms",
        trace.model.as_deref().unwrap_or("-"),
        trace.model_call_count,
        trace.attached_tool_count,
        trace.tool_steps,
        trace.clarification_count,
        trace.latency_ms
    );
    let _ = app.emit(
        "chat-trace",
        json!({
            "step": "done",
            "detail": detail,
            "path": path,
            "model": trace.model,
            "model_call_count": trace.model_call_count,
            "attached_tool_count": trace.attached_tool_count,
            "tool_steps": trace.tool_steps,
            "clarification_count": trace.clarification_count,
            "latency_ms": trace.latency_ms,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_and_extract_need_no_model() {
        assert!(path_expects_zero_model_calls(TurnPath::Canonical));
        assert!(path_expects_zero_model_calls(TurnPath::Extract));
        assert!(path_expects_zero_model_calls(TurnPath::LastLook));
        assert!(!path_expects_zero_model_calls(TurnPath::TalkLlama));
        assert!(!path_expects_zero_model_calls(TurnPath::CompleteQwen));
        assert!(!path_expects_zero_model_calls(TurnPath::ResumeNative));
    }

    #[test]
    fn trivial_chat_uses_llama_only_when_qwen_is_cold() {
        assert_eq!(talk_policy_path(true, false), TurnPath::TalkLlama);
        assert_eq!(talk_policy_path(true, true), TurnPath::CompleteQwen);
        assert_eq!(talk_policy_path(false, false), TurnPath::CompleteQwen);
    }

    #[test]
    fn set_path_keeps_first_winner() {
        let mut trace = TurnTrace::new("c1");
        trace.set_path(TurnPath::TalkRespond);
        trace.set_path(TurnPath::TalkLlama);
        assert_eq!(trace.path, Some(TurnPath::TalkRespond));
    }
}
