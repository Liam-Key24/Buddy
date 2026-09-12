//! Persist calendar organize proposals and emit ghost-week UI events.

use serde_json::json;
use tauri::{AppHandle, Emitter};

use buddy_calendar::OrganizeMode;
use buddy_personality::{phrase_tool_result, style_response, PersonalityProfile};

use crate::state::AppState;
use crate::work_item::PendingApproval;

pub use buddy_calendar::{proposal_from_organize, StoredCalendarProposal};

const LAST_PROPOSAL_KEY: &str = "calendar_proposal:last";

fn conv_key(conversation_id: &str) -> String {
    format!("calendar_proposal:{conversation_id}")
}

pub fn save_proposal(state: &AppState, proposal: &StoredCalendarProposal) {
    if let Ok(raw) = serde_json::to_string(proposal) {
        let _ = state
            .db
            .set_runtime_state(&conv_key(&proposal.conversation_id), &raw);
        let _ = state.db.set_runtime_state(LAST_PROPOSAL_KEY, &raw);
    }
    if !proposal.conversation_id.trim().is_empty() {
        state.memory.attach_approval(
            &proposal.conversation_id,
            PendingApproval {
                kind: "calendar_proposal".into(),
                tool: Some("calendar.organize".into()),
                summary: Some(format!("{} proposed blocks", proposal.blocks.len())),
            },
        );
    }
}

pub fn load_proposal(
    state: &AppState,
    conversation_id: Option<&str>,
) -> Option<StoredCalendarProposal> {
    let raw = conversation_id
        .and_then(|id| state.db.get_runtime_state(&conv_key(id)).ok().flatten())
        .or_else(|| state.db.get_runtime_state(LAST_PROPOSAL_KEY).ok().flatten())?;
    if raw.trim().is_empty() {
        return None;
    }
    serde_json::from_str(&raw).ok()
}

pub fn clear_proposal(state: &AppState, app: &AppHandle, conversation_id: Option<&str>) {
    if let Some(id) = conversation_id {
        let _ = state.db.delete_runtime_state(&conv_key(id));
    }
    if let Some(last) = load_proposal(state, None) {
        if conversation_id.is_none() || conversation_id == Some(last.conversation_id.as_str()) {
            let _ = state.db.delete_runtime_state(LAST_PROPOSAL_KEY);
        }
    } else {
        let _ = state.db.delete_runtime_state(LAST_PROPOSAL_KEY);
    }
    if let Some(id) = conversation_id {
        state.memory.clear_approval(id);
    }
    let _ = app.emit(
        "calendar-proposal",
        json!({ "blocks": [], "cleared": true }),
    );
}

pub fn emit_proposal(app: &AppHandle, proposal: &StoredCalendarProposal) {
    let _ = app.emit(
        "calendar-proposal",
        json!({
            "conversation_id": proposal.conversation_id,
            "blocks": proposal.blocks,
            "cleared": false,
        }),
    );
}

pub async fn commit_stored_proposal(
    app: &AppHandle,
    state: &AppState,
    conversation_id: Option<&str>,
    personality: Option<&PersonalityProfile>,
) -> Result<String, String> {
    let proposal = load_proposal(state, conversation_id)
        .ok_or_else(|| "No calendar proposal to accept.".to_string())?;
    let result = state
        .calendar
        .organize(
            proposal.window.clone(),
            proposal.items.clone(),
            proposal.constraints.clone(),
            OrganizeMode::Commit,
            Some(proposal.items.clone()),
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
    clear_proposal(state, app, Some(&proposal.conversation_id));
    if let Some(cid) = conversation_id.or(Some(proposal.conversation_id.as_str())) {
        state.memory.clear_pending_clarification(cid);
        state.memory.clear_agent_turn(cid);
    }
    let _ = app.emit("calendar-updated", ());
    let output = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".into());
    let summary = phrase_tool_result("calendar.organize", &output);
    let content = if let Some(p) = personality {
        style_response(p, &summary)
    } else {
        summary
    };
    let _ = app.emit("chat-chunk", &content);
    Ok(content)
}
