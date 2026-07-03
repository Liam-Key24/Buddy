mod commands;
mod intelligence_hooks;
mod logging;
mod memory_extraction;
mod orchestrator;
mod services;
mod spark_checker;
mod state;

use std::sync::Arc;

use buddy_database::Database;
use buddy_memory::MemoryContext;
use memory_extraction::session_end_handover;
use services::ProcessManager;
use state::{db_path, find_project_root, logs_dir, AppState};
use tauri::{Manager, RunEvent};
use tracing::info;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let project_root = find_project_root();
    let logs_dir = logs_dir();
    logging::init_logging(&logs_dir, "info");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(move |app| {
            let path = db_path(app.handle());
            info!(path = %path.display(), "opening database");
            let db = Database::open(&path).expect("failed to open database");
            let state = Arc::new(AppState::new(db, project_root.clone()));
            let process_manager = Arc::new(ProcessManager::new());

            app.manage(state.clone());
            app.manage(process_manager.clone());

            spark_checker::spawn_spark_checker(app.handle().clone(), state.clone());

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
                let ctx = MemoryContext {
                    workspace_path: st.project_root.clone(),
                    conversation_id: None,
                    task_id: None,
                };
                st.intelligence.spawn_reindex(ctx).await;
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
        let _ = state.intelligence.run_maintenance(&ctx).await;
    });
}
