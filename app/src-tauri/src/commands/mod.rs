use std::sync::Arc;

use buddy_core::ToolResult;
use serde::Serialize;
use tauri::{Emitter, State};

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
    pub model_name_chat: String,
    pub model_name_code: String,
    pub llm_profile_router: String,
    pub codex_model: String,
    pub codex_workspace: String,
    pub code_agent_backend: String,
    pub code_model: String,
    pub cursor_path: String,
    pub codex_path: String,
    pub email_signature: String,
    pub email_greeting: String,
    pub email_body_template: String,
    pub fs_excluded_paths: Vec<String>,
    pub calendar_provider: String,
    pub calcom_base_url: String,
    pub calcom_api_version: String,
    pub calcom_event_type_id: String,
    pub calcom_username: String,
    pub calcom_timezone: String,
    pub calendar_default_duration_min: String,
    pub calendar_auto_create_threshold: String,
    pub calendar_working_windows: String,
    pub calendar_min_focus_min: String,
    pub calendar_move_horizon_hours: String,
}

fn setting_or(state: &AppState, key: &str, default: &str) -> String {
    state
        .db
        .get_setting(key)
        .ok()
        .flatten()
        .unwrap_or_else(|| default.to_string())
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
    const DEFAULT_MODEL: &str = "mlx-community/Llama-3.2-3B-Instruct-4bit";
    let model_name = setting_or(&state, "model_name", DEFAULT_MODEL);
    let fs_excluded_paths = state
        .db
        .get_setting("fs_excluded_paths")
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
        .unwrap_or_default();

    Ok(SettingsMap {
        mlx_url: state.mlx_url(),
        brain_url: state.brain_url(),
        log_level: setting_or(&state, "log_level", "info"),
        auto_start_mlx: setting_or(&state, "auto_start_mlx", "false") == "true",
        model_name_chat: setting_or(&state, "model_name_chat", &model_name),
        model_name_code: setting_or(&state, "model_name_code", &model_name),
        llm_profile_router: setting_or(&state, "llm_profile_router", &model_name),
        codex_model: setting_or(&state, "codex_model", "gpt-5.5"),
        codex_workspace: setting_or(
            &state,
            "codex_workspace",
            &state.project_root.display().to_string(),
        ),
        code_agent_backend: setting_or(&state, "code_agent_backend", "cursor"),
        code_model: setting_or(&state, "code_model", "auto"),
        cursor_path: setting_or(&state, "cursor_path", ""),
        codex_path: setting_or(&state, "codex_path", ""),
        email_signature: setting_or(&state, "email_signature", ""),
        email_greeting: setting_or(&state, "email_greeting", "Hi,"),
        email_body_template: setting_or(
            &state,
            "email_body_template",
            "{greeting}\n\n{body}\n\n{signature}",
        ),
        calendar_provider: setting_or(&state, "calendar_provider", "calcom_self_hosted"),
        calcom_base_url: setting_or(&state, "calcom_base_url", ""),
        calcom_api_version: setting_or(&state, "calcom_api_version", "2024-08-13"),
        calcom_event_type_id: setting_or(&state, "calcom_event_type_id", ""),
        calcom_username: setting_or(&state, "calcom_username", ""),
        calcom_timezone: setting_or(&state, "calcom_timezone", ""),
        calendar_default_duration_min: setting_or(&state, "calendar_default_duration_min", "30"),
        calendar_auto_create_threshold: setting_or(
            &state,
            "calendar_auto_create_threshold",
            "0.85",
        ),
        calendar_working_windows: setting_or(
            &state,
            "calendar_working_windows",
            "09:00-12:00,14:00-18:00",
        ),
        calendar_min_focus_min: setting_or(&state, "calendar_min_focus_min", "90"),
        calendar_move_horizon_hours: setting_or(&state, "calendar_move_horizon_hours", "48"),
        fs_excluded_paths,
        model_name,
    })
}

#[tauri::command]
pub fn set_setting(state: State<'_, Arc<AppState>>, key: String, value: String) -> Result<(), String> {
    state.db.set_setting(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_sparks(
    state: State<'_, Arc<AppState>>,
    status: Option<String>,
) -> Result<Vec<buddy_database::Spark>, String> {
    state
        .db
        .list_sparks(status.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_spark(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    content: String,
    tags: Vec<String>,
) -> Result<buddy_database::Spark, String> {
    let spark = state
        .db
        .create_spark(&content, &tags, None)
        .map_err(|e| e.to_string())?;
    let count = state
        .db
        .count_stale_sparks(
            buddy_database::SPARK_STALE_AGE_MS,
            buddy_database::SPARK_NUDGE_COOLDOWN_MS,
        )
        .unwrap_or(0);
    let _ = app.emit("sparks-stale", count);
    let _ = app.emit("sparks-updated", ());
    Ok(spark)
}

#[tauri::command]
pub fn update_spark(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
    action: String,
    content: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<buddy_database::Spark, String> {
    let spark = state
        .db
        .update_spark(&id, &action, content.as_deref(), tags.as_deref())
        .map_err(|e| e.to_string())?;
    let count = state
        .db
        .count_stale_sparks(
            buddy_database::SPARK_STALE_AGE_MS,
            buddy_database::SPARK_NUDGE_COOLDOWN_MS,
        )
        .unwrap_or(0);
    let _ = app.emit("sparks-stale", count);
    let _ = app.emit("sparks-updated", ());
    Ok(spark)
}

#[tauri::command]
pub async fn delete_spark(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    orchestrator::delete_spark_with_archive(state.inner(), &app, &id).await
}

#[tauri::command]
pub fn get_stale_spark_count(state: State<'_, Arc<AppState>>) -> Result<i64, String> {
    state
        .db
        .count_stale_sparks(
            buddy_database::SPARK_STALE_AGE_MS,
            buddy_database::SPARK_NUDGE_COOLDOWN_MS,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_stale_sparks(state: State<'_, Arc<AppState>>) -> Result<Vec<buddy_database::Spark>, String> {
    state
        .db
        .get_stale_sparks(
            buddy_database::SPARK_STALE_AGE_MS,
            buddy_database::SPARK_NUDGE_COOLDOWN_MS,
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_secret(key: String, value: String) -> Result<(), String> {
    if !crate::secrets::KNOWN_SECRETS.contains(&key.as_str()) {
        return Err(format!("unknown secret key: {key}"));
    }
    crate::secrets::set_secret(&key, &value)
}

#[tauri::command]
pub fn delete_secret(key: String) -> Result<(), String> {
    if !crate::secrets::KNOWN_SECRETS.contains(&key.as_str()) {
        return Err(format!("unknown secret key: {key}"));
    }
    crate::secrets::delete_secret(&key)
}

#[tauri::command]
pub fn get_secret_status() -> Result<std::collections::HashMap<String, bool>, String> {
    Ok(crate::secrets::KNOWN_SECRETS
        .iter()
        .map(|key| (key.to_string(), crate::secrets::has_secret(key)))
        .collect())
}

#[tauri::command]
pub fn list_external_actions(
    state: State<'_, Arc<AppState>>,
    limit: Option<usize>,
) -> Result<Vec<buddy_database::ExternalAction>, String> {
    state
        .db
        .list_external_actions(limit.unwrap_or(100))
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct RefreshCacheResult {
    pub memories_reindexed: usize,
    pub duration_ms: u64,
}

#[tauri::command]
pub async fn refresh_cache(
    state: State<'_, Arc<AppState>>,
) -> Result<RefreshCacheResult, String> {
    let start = std::time::Instant::now();
    let ctx = buddy_memory::MemoryContext {
        workspace_path: state.project_root.clone(),
        conversation_id: None,
        task_id: None,
    };

    let memories_reindexed = state
        .intelligence
        .reindex_workspace(&ctx)
        .await
        .map_err(|e| e.to_string())?;
    let _ = state.intelligence.run_maintenance(&ctx).await;

    let duration_ms = start.elapsed().as_millis() as u64;
    let _ = state.db.log_external_action(
        "cache_refresh",
        &format!("Reindexed {memories_reindexed} memories in {duration_ms}ms"),
        None,
        true,
    );

    Ok(RefreshCacheResult {
        memories_reindexed,
        duration_ms,
    })
}

#[tauri::command]
pub fn list_codex_conversations(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<buddy_database::Conversation>, String> {
    state
        .db
        .list_conversations_by_kind(Some("codex"))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_codex_conversation(
    state: State<'_, Arc<AppState>>,
    title: Option<String>,
    focus_mode: Option<String>,
    workspace_path: Option<String>,
) -> Result<buddy_database::Conversation, String> {
    state
        .db
        .create_conversation_with_kind(
            &title.unwrap_or_else(|| "New project".to_string()),
            "codex",
            Some(focus_mode.as_deref().unwrap_or("planning")),
            workspace_path.as_deref(),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_conversation_focus(
    state: State<'_, Arc<AppState>>,
    id: String,
    focus_mode: String,
) -> Result<(), String> {
    state
        .db
        .set_conversation_focus(&id, &focus_mode)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_codex_message(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    conversation_id: String,
    text: String,
    focus: Option<String>,
    attachments: Option<Vec<String>>,
) -> Result<(), String> {
    crate::codex_orchestrator::send_codex_message(
        app,
        &state,
        conversation_id,
        text,
        focus,
        attachments.unwrap_or_default(),
    )
    .await
}

#[tauri::command]
pub fn terminal_open(
    app: tauri::AppHandle,
    manager: State<'_, Arc<crate::terminal::TerminalManager>>,
    cwd: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<String, String> {
    manager.open(
        app,
        &cwd.unwrap_or_default(),
        cols.unwrap_or(80),
        rows.unwrap_or(24),
    )
}

#[tauri::command]
pub fn terminal_write(
    manager: State<'_, Arc<crate::terminal::TerminalManager>>,
    id: String,
    data: String,
) -> Result<(), String> {
    manager.write(&id, &data)
}

#[tauri::command]
pub fn terminal_resize(
    manager: State<'_, Arc<crate::terminal::TerminalManager>>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    manager.resize(&id, cols, rows)
}

#[tauri::command]
pub fn terminal_close(
    manager: State<'_, Arc<crate::terminal::TerminalManager>>,
    id: String,
) -> Result<(), String> {
    manager.close(&id);
    Ok(())
}
