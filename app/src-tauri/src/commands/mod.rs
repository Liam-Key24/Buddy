use std::sync::Arc;

use buddy_core::{excluded_paths_from_setting, ToolResult};
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
    pub calendar_notifications_enabled: bool,
    pub calendar_default_timezone: String,
    pub calendar_default_reminders_json: String,
}

fn setting_or(state: &AppState, key: &str, default: &str) -> String {
    state.db.get_setting_or(key, default)
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
    let fs_excluded_paths =
        excluded_paths_from_setting(state.db.get_setting("fs_excluded_paths").ok().flatten());

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
        fs_excluded_paths,
        model_name,
        calendar_notifications_enabled: setting_or(
            &state,
            "calendar_notifications_enabled",
            "true",
        ) == "true",
        calendar_default_timezone: setting_or(&state, "calendar_default_timezone", "UTC"),
        calendar_default_reminders_json: setting_or(
            &state,
            "calendar_default_reminders_json",
            "[{\"minutes_before\":15,\"method\":\"popup\"}]",
        ),
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
    if !crate::secrets::is_known_secret(&key) {
        return Err(format!("unknown secret key: {key}"));
    }
    crate::secrets::set_secret(&key, &value)
}

#[tauri::command]
pub fn delete_secret(key: String) -> Result<(), String> {
    if !crate::secrets::is_known_secret(&key) {
        return Err(format!("unknown secret key: {key}"));
    }
    crate::secrets::delete_secret(&key)
}

#[tauri::command]
pub fn get_secret_status() -> Result<std::collections::HashMap<String, bool>, String> {
    Ok(crate::secrets::known_secrets()
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

    let memories_reindexed = state.memory.reindex_workspace().await?;
    let _ = state.memory.run_global_maintenance().await;

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
    use crate::coder_bridge::AppEmit;
    use buddy_coder::CodeEmit;

    let emit = AppEmit(app);
    let conversation = state
        .db
        .get_conversation(&conversation_id)
        .map_err(|e| e.to_string())?;
    let resolved_focus = focus
        .clone()
        .or(conversation.focus_mode.clone())
        .unwrap_or_else(|| "planning".to_string());
    let attachments = attachments.unwrap_or_default();

    let user_metadata = serde_json::json!({
        "attachments": &attachments,
        "focus": &resolved_focus,
    })
    .to_string();
    state
        .db
        .add_message_with_metadata(&conversation_id, "user", &text, Some(&user_metadata))
        .map_err(|e| e.to_string())?;

    if state
        .db
        .get_messages(&conversation_id)
        .map(|m| m.len())
        .unwrap_or(0)
        == 1
    {
        let title: String = text.chars().take(40).collect();
        let _ = state
            .db
            .update_conversation_title(&conversation_id, &title);
    }

    let input = serde_json::json!({
        "conversation_id": &conversation_id,
        "prompt": &text,
        "focus": focus,
        "attachments": &attachments,
    })
    .to_string();

    // Same Core pipeline as chat: TaskRunner → coder.run plugin.
    let result = tokio::task::block_in_place(|| state.task_runner.run("coder.run", &input))
        .map_err(|e| e.to_string())?;

    emit.chunk(&result.output);

    let assistant_metadata = serde_json::json!({
        "backend": "coder.run",
        "focus": resolved_focus,
        "attachments": attachments.len(),
    })
    .to_string();
    state
        .db
        .add_message_with_metadata(
            &conversation_id,
            "assistant",
            &result.output,
            Some(&assistant_metadata),
        )
        .map_err(|e| e.to_string())?;

    emit.done();
    Ok(())
}

#[tauri::command]
pub fn terminal_open(
    app: tauri::AppHandle,
    manager: State<'_, Arc<buddy_coder::TerminalManager>>,
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
    manager: State<'_, Arc<buddy_coder::TerminalManager>>,
    id: String,
    data: String,
) -> Result<(), String> {
    manager.write(&id, &data)
}

#[tauri::command]
pub fn terminal_resize(
    manager: State<'_, Arc<buddy_coder::TerminalManager>>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    manager.resize(&id, cols, rows)
}

#[tauri::command]
pub fn terminal_close(
    manager: State<'_, Arc<buddy_coder::TerminalManager>>,
    id: String,
) -> Result<(), String> {
    manager.close(&id);
    Ok(())
}

fn calendar_err(e: buddy_calendar::CalendarError) -> String {
    format!("{}: {}", e.code(), e)
}

#[tauri::command]
pub async fn calendar_list_events(
    state: State<'_, Arc<AppState>>,
    start: i64,
    end: i64,
    query: Option<String>,
    categories: Option<Vec<String>>,
) -> Result<Vec<buddy_calendar::Event>, String> {
    state
        .calendar
        .list_events(
            buddy_calendar::DateRange { start, end },
            buddy_calendar::EventFilters {
                query,
                categories: categories.unwrap_or_default(),
            },
        )
        .await
        .map_err(calendar_err)
}

#[tauri::command]
pub async fn calendar_get_event(
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<buddy_calendar::Event, String> {
    state.calendar.get_event(&id).await.map_err(calendar_err)
}

#[tauri::command]
pub async fn calendar_create_event(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    mut input: buddy_calendar::CreateEventInput,
) -> Result<buddy_calendar::Event, String> {
    // UI-driven creates are explicit user choices — allow writing after validation.
    input.force = true;
    let event = state
        .calendar
        .create_event(input)
        .await
        .map_err(calendar_err)?;
    let _ = app.emit("calendar-updated", ());
    Ok(event)
}

#[tauri::command]
pub async fn calendar_update_event(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
    mut input: buddy_calendar::UpdateEventInput,
) -> Result<buddy_calendar::Event, String> {
    input.force = true;
    let event = state
        .calendar
        .update_event(&id, input)
        .await
        .map_err(calendar_err)?;
    let _ = app.emit("calendar-updated", ());
    Ok(event)
}

#[tauri::command]
pub async fn calendar_delete_event(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state
        .calendar
        .delete_event(&id)
        .await
        .map_err(calendar_err)?;
    let _ = app.emit("calendar-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn calendar_duplicate_event(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<buddy_calendar::Event, String> {
    let event = state
        .calendar
        .duplicate_event(&id)
        .await
        .map_err(calendar_err)?;
    let _ = app.emit("calendar-updated", ());
    Ok(event)
}

#[tauri::command]
pub async fn calendar_search_events(
    state: State<'_, Arc<AppState>>,
    query: String,
    start: Option<i64>,
    end: Option<i64>,
) -> Result<Vec<buddy_calendar::Event>, String> {
    let range = match (start, end) {
        (Some(start), Some(end)) => Some(buddy_calendar::DateRange { start, end }),
        _ => None,
    };
    state
        .calendar
        .search_events(&query, range)
        .await
        .map_err(calendar_err)
}

#[tauri::command]
pub async fn calendar_get_today(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<buddy_calendar::Event>, String> {
    state.calendar.get_today().await.map_err(calendar_err)
}

#[tauri::command]
pub async fn calendar_get_tomorrow(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<buddy_calendar::Event>, String> {
    state.calendar.get_tomorrow().await.map_err(calendar_err)
}

#[tauri::command]
pub async fn calendar_get_this_week(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<buddy_calendar::Event>, String> {
    state.calendar.get_this_week().await.map_err(calendar_err)
}

#[tauri::command]
pub async fn calendar_list_notifications(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<buddy_calendar::ReminderDelivery>, String> {
    state
        .calendar
        .list_notifications()
        .await
        .map_err(calendar_err)
}

#[tauri::command]
pub async fn calendar_snooze_reminder(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
    minutes: u32,
) -> Result<(), String> {
    state
        .calendar
        .snooze_reminder(&id, minutes)
        .await
        .map_err(calendar_err)?;
    if let Ok(count) = state.calendar.notification_count().await {
        let _ = app.emit("calendar-notification-count", count);
    }
    Ok(())
}

#[tauri::command]
pub async fn calendar_dismiss_reminder(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state
        .calendar
        .dismiss_reminder(&id)
        .await
        .map_err(calendar_err)?;
    if let Ok(count) = state.calendar.notification_count().await {
        let _ = app.emit("calendar-notification-count", count);
    }
    Ok(())
}

#[tauri::command]
pub async fn calendar_notification_count(
    state: State<'_, Arc<AppState>>,
) -> Result<i64, String> {
    state
        .calendar
        .notification_count()
        .await
        .map_err(calendar_err)
}

#[tauri::command]
pub async fn lifestyle_list_blocks(
    state: State<'_, Arc<AppState>>,
    start: i64,
    end: i64,
) -> Result<Vec<buddy_calendar::ScheduleBlock>, String> {
    state
        .calendar
        .list_schedule_blocks(start, end)
        .await
        .map_err(calendar_err)
}

#[tauri::command]
pub async fn dream_list(
    state: State<'_, Arc<AppState>>,
    sleep_date: String,
) -> Result<Vec<buddy_calendar::DreamEntry>, String> {
    state
        .calendar
        .list_dreams(&sleep_date)
        .await
        .map_err(calendar_err)
}

#[tauri::command]
pub async fn dream_log(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    input: buddy_calendar::CreateDreamInput,
) -> Result<buddy_calendar::DreamEntry, String> {
    let dream = state.calendar.log_dream(input).await.map_err(calendar_err)?;
    let _ = app.emit("calendar-updated", ());
    Ok(dream)
}

#[tauri::command]
pub async fn dream_update(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
    input: buddy_calendar::UpdateDreamInput,
) -> Result<buddy_calendar::DreamEntry, String> {
    let dream = state
        .calendar
        .update_dream(&id, input)
        .await
        .map_err(calendar_err)?;
    let _ = app.emit("calendar-updated", ());
    Ok(dream)
}

#[tauri::command]
pub async fn dream_delete(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<(), String> {
    state.calendar.delete_dream(&id).await.map_err(calendar_err)?;
    let _ = app.emit("calendar-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn dream_search(
    state: State<'_, Arc<AppState>>,
    query: String,
) -> Result<Vec<buddy_calendar::DreamEntry>, String> {
    state
        .calendar
        .search_dreams(&query)
        .await
        .map_err(calendar_err)
}

#[tauri::command]
pub async fn work_get_stats(
    state: State<'_, Arc<AppState>>,
) -> Result<buddy_calendar::WorkStats, String> {
    state.calendar.get_work_stats().await.map_err(calendar_err)
}

#[tauri::command]
pub async fn work_log_sales(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    amount: f64,
    work_date: Option<String>,
    currency: Option<String>,
) -> Result<buddy_calendar::WorkDayLog, String> {
    let log = state
        .calendar
        .log_work_sales(work_date, amount, currency)
        .await
        .map_err(calendar_err)?;
    let _ = app.emit("calendar-updated", ());
    Ok(log)
}

#[tauri::command]
pub async fn work_set_hours(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    work_date: Option<String>,
    actual_start_ms: Option<i64>,
    actual_end_ms: Option<i64>,
) -> Result<buddy_calendar::WorkDayLog, String> {
    let log = state
        .calendar
        .set_work_hours(work_date, actual_start_ms, actual_end_ms)
        .await
        .map_err(calendar_err)?;
    let _ = app.emit("calendar-updated", ());
    Ok(log)
}

#[tauri::command]
pub async fn work_get_day_log(
    state: State<'_, Arc<AppState>>,
    work_date: String,
) -> Result<buddy_calendar::WorkDayLog, String> {
    state
        .calendar
        .get_work_day_log(&work_date)
        .await
        .map_err(calendar_err)
}

#[tauri::command]
pub async fn lifestyle_last_sleep_date(
    state: State<'_, Arc<AppState>>,
) -> Result<String, String> {
    state
        .calendar
        .last_sleep_date()
        .await
        .map_err(calendar_err)
}

#[tauri::command]
pub async fn calendar_get_capacity(
    state: State<'_, Arc<AppState>>,
    day: i64,
) -> Result<buddy_calendar::DayCapacity, String> {
    state.calendar.get_capacity(day).await.map_err(calendar_err)
}

#[tauri::command]
pub async fn calendar_day_summary(
    state: State<'_, Arc<AppState>>,
    day: i64,
) -> Result<buddy_calendar::DaySummary, String> {
    state.calendar.day_summary(day).await.map_err(calendar_err)
}
