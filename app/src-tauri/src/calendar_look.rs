//! Last-look follow-ups: persist snapshot + phrase without a model round.

use buddy_calendar::{last_look_from_tool_output, select_followup_payload};
use buddy_personality::{phrase_tool_result, style_response, PersonalityProfile};
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::state::AppState;

pub fn remember_look(state: &AppState, conversation_id: &str, output: &str) {
    if let Some(look) = last_look_from_tool_output(output) {
        if let Ok(raw) = serde_json::to_string(&look) {
            state.memory.set_last_look_raw(conversation_id, &raw);
        }
    }
}

pub fn try_resolve(
    app: &AppHandle,
    state: &AppState,
    conversation_id: &str,
    personality: &PersonalityProfile,
    text: &str,
) -> Option<String> {
    let look = serde_json::from_str(&state.memory.get_last_look_raw(conversation_id)?).ok()?;
    let payload = select_followup_payload(&look, text)?;
    let _ = app.emit(
        "chat-trace",
        json!({ "step": "running", "detail": "Last look follow-up" }),
    );
    let content = style_response(
        personality,
        &phrase_tool_result("calendar.look", &payload.to_string()),
    );
    let _ = app.emit("chat-chunk", &content);
    Some(content)
}
