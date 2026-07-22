mod calendar_bridge;
mod calendar_reminder_checker;
mod coder_bridge;
mod coder_tool;
mod commands;
mod intelligence_hooks;
mod logging;
mod memory_api;
mod memory_extraction;
mod memory_tools;
mod orchestrator;
mod secrets;
mod services;
mod spark_checker;
mod state;

use std::sync::Arc;

use buddy_coder::TerminalManager;
use buddy_database::Database;
use buddy_memory::MemoryContext;
use memory_extraction::session_end_handover;
use services::ProcessManager;
use state::{db_path, find_project_root, logs_dir, AppState};
use tauri::{Manager, RunEvent};
use tracing::{error, info, warn};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let project_root = find_project_root();
    let logs_dir = logs_dir();
    logging::init_logging(&logs_dir, "info");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let path = db_path(app.handle());
            info!(path = %path.display(), "opening database");
            let db = match Database::open(&path) {
                Ok(db) => db,
                Err(e) => {
                    error!(error = %e, path = %path.display(), "database open failed");
                    return Err(format!(
                        "Could not open Buddy database at {}: {e}. Try quitting other Buddy instances or repairing buddy.db.",
                        path.display()
                    )
                    .into());
                }
            };
            let state = AppState::new(db, project_root.clone());
            let process_manager = Arc::new(ProcessManager::new());

            app.manage(state.clone());
            app.manage(process_manager.clone());
            app.manage(Arc::new(TerminalManager::new()));

            spark_checker::spawn_spark_checker(app.handle().clone(), state.clone());
            calendar_reminder_checker::spawn_calendar_reminder_checker(
                app.handle().clone(),
                state.clone(),
            );

            let pm = process_manager.clone();
            let st = state.clone();
            tauri::async_runtime::spawn(async move {
                for _ in 0..10 {
                    if ProcessManager::check_brain_ready(&st).await {
                        break;
                    }
                    let _ = pm.ensure_brain(&st).await;
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                st.memory.spawn_reindex().await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_service_status,
            commands::start_brain,
            commands::list_conversations,
            commands::create_conversation,
            commands::delete_conversation,
            commands::get_messages,
            commands::send_message,
            commands::run_tool,
            commands::get_settings,
            commands::set_setting,
            commands::list_sparks,
            commands::create_spark,
            commands::update_spark,
            commands::delete_spark,
            commands::get_stale_spark_count,
            commands::get_stale_sparks,
            commands::set_secret,
            commands::delete_secret,
            commands::get_secret_status,
            commands::list_external_actions,
            commands::refresh_cache,
            commands::list_codex_conversations,
            commands::create_codex_conversation,
            commands::set_conversation_focus,
            commands::send_codex_message,
            commands::terminal_open,
            commands::terminal_write,
            commands::terminal_resize,
            commands::terminal_close,
            commands::calendar_list_events,
            commands::calendar_get_event,
            commands::calendar_create_event,
            commands::calendar_update_event,
            commands::calendar_delete_event,
            commands::calendar_duplicate_event,
            commands::calendar_search_events,
            commands::calendar_get_today,
            commands::calendar_get_tomorrow,
            commands::calendar_get_this_week,
            commands::calendar_list_notifications,
            commands::calendar_snooze_reminder,
            commands::calendar_dismiss_reminder,
            commands::calendar_notification_count,
            commands::lifestyle_list_blocks,
            commands::lifestyle_last_sleep_date,
            commands::dream_list,
            commands::dream_log,
            commands::dream_update,
            commands::dream_delete,
            commands::dream_search,
            commands::work_get_stats,
            commands::work_log_sales,
            commands::work_set_hours,
            commands::work_get_day_log,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                if let Some(state) = app.try_state::<Arc<AppState>>() {
                    handle_exit(state.inner().clone());
                }
            }
        });
}

fn handle_exit(state: Arc<AppState>) {
    let ctx = MemoryContext {
        workspace_path: state.project_root.clone(),
        conversation_id: None,
        task_id: None,
    };
    tauri::async_runtime::block_on(async {
        session_end_handover(&state, &ctx).await;
        if let Err(e) = state.memory.run_global_maintenance().await {
            warn!(error = %e, "exit maintenance failed");
        }
    });
}
