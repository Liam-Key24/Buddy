use std::sync::Arc;

use buddy_core::ToolResult;
use serde::Serialize;
use tauri::State;

use crate::orchestrator;
use crate::services::{ProcessManager, ServiceStatus};
use crate::state::AppState;

#[derive(Serialize)]
pub struct SettingsMap {
    pub mlx_url: String,
    pub brain_url: String,
    pub model_name: String,
    pub log_level: String,
    pub auto_start_mlx: bool,
}

#[tauri::command]
pub async fn get_service_status(state: State<'_, Arc<AppState>>) -> Result<ServiceStatus, String> {
    Ok(ProcessManager::get_status(&state).await)
}

#[tauri::command]
pub fn start_brain(
    process_manager: State<'_, Arc<ProcessManager>>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    process_manager.start_brain(&state)
}

#[tauri::command]
pub fn list_conversations(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<buddy_database::Conversation>, String> {
    state.db.list_conversations().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_conversation(
    state: State<'_, Arc<AppState>>,
    title: Option<String>,
) -> Result<buddy_database::Conversation, String> {
    state
        .db
        .create_conversation(&title.unwrap_or_else(|| "New chat".to_string()))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_conversation(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    orchestrator::delete_conversation(state.inner(), &id).await
}

#[tauri::command]
pub fn get_messages(
    state: State<'_, Arc<AppState>>,
    conversation_id: String,
) -> Result<Vec<buddy_database::Message>, String> {
    state
        .db
        .get_messages(&conversation_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_message(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    conversation_id: String,
    text: String,
) -> Result<(), String> {
    orchestrator::send_message(app, &state, conversation_id, text).await
}

#[tauri::command]
pub fn run_tool(
    state: State<'_, Arc<AppState>>,
    name: String,
    input: String,
) -> Result<ToolResult, String> {
    state.task_runner.run(&name, &input).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(state: State<'_, Arc<AppState>>) -> Result<SettingsMap, String> {
    let log_level = state
        .db
        .get_setting("log_level")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "info".to_string());
    let model_name = state
        .db
        .get_setting("model_name")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "mlx-community/Llama-3.2-3B-Instruct-4bit".to_string());
    let auto_start_mlx = state
        .db
        .get_setting("auto_start_mlx")
        .map_err(|e| e.to_string())?
        .map(|v| v == "true")
        .unwrap_or(false);

    Ok(SettingsMap {
        mlx_url: state.mlx_url(),
        brain_url: state.brain_url(),
        model_name,
        log_level,
        auto_start_mlx,
    })
}

#[tauri::command]
pub fn set_setting(state: State<'_, Arc<AppState>>, key: String, value: String) -> Result<(), String> {
    state.db.set_setting(&key, &value).map_err(|e| e.to_string())
}
