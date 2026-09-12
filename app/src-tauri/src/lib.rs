mod calendar_bridge;
mod calendar_look;
mod calendar_proposal;
mod calendar_reminder_checker;
mod native_loop;
mod coder_bridge;
mod coder_tool;
mod commands;
mod intelligence_hooks;
mod logging;
mod memory_api;
mod memory_extraction;
mod memory_tools;
mod orchestrator;
mod run_control;
mod secrets;
mod services;
mod skills;
mod socials_generate;
mod spark_checker;
mod state;
mod turn_controller;
mod turn_trace;
mod work_item;

use std::sync::Arc;

use buddy_coder::TerminalManager;
use buddy_database::Database;
use buddy_memory::MemoryContext;
use memory_extraction::session_end_handover;
use services::ProcessManager;
use state::{db_path, logs_dir, resolve_project_root, AppState};
use tauri::{Manager, RunEvent};
use tracing::{error, info, warn};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
            let project_root = resolve_project_root(&db);
            info!(path = %project_root.display(), "project root");
            if !ProcessManager::project_root_looks_valid(&project_root) {
                warn!(
                    path = %project_root.display(),
                    "project root does not look like the Buddy repo — Brain/MLX auto-start may fail. Set BUDDY_PROJECT_ROOT or launch once from the repo."
                );
            }

            let state = AppState::new(db, project_root.clone());
            let process_manager = Arc::new(ProcessManager::new());
            process_manager.reclaim_mlx_port(&state);
            process_manager.spawn_idle_watcher(state.clone());

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
                info!("mlx starts only when a chat turn needs the model");

                for _ in 0..3 {
                    if ProcessManager::check_brain_ready(&st).await {
                        info!("brain ready");
                        break;
                    }
                    if let Err(e) = pm.ensure_brain(&st).await {
                        warn!(error = %e, "brain auto-start failed");
                    } else {
                        info!("brain ready");
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                st.memory.spawn_reindex().await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_service_status,
            commands::start_brain,
            commands::restart_brain,
            commands::start_mlx,
            commands::restart_mlx,
            commands::start_runtime,
            commands::list_conversations,
            commands::create_conversation,
            commands::delete_conversation,
            commands::get_messages,
            commands::send_message,
            commands::stop_run,
            commands::resolve_clarification,
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
            commands::calendar_get_capacity,
            commands::calendar_day_summary,
            commands::calendar_commit_proposal,
            commands::calendar_dismiss_proposal,
            commands::lifestyle_list_blocks,
            commands::lifestyle_list_rules,
            commands::lifestyle_set_times,
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
            commands::life::todo_list,
            commands::life::todo_upsert,
            commands::life::todo_complete,
            commands::life::todo_delete,
            commands::life::docs_list_folders,
            commands::life::docs_upsert_folder,
            commands::life::docs_delete_folder,
            commands::life::docs_list,
            commands::life::docs_get,
            commands::life::docs_upsert,
            commands::life::docs_delete,
            commands::life::docs_search,
            commands::life::create_research_conversation,
            commands::life::research_list,
            commands::life::research_get,
            commands::life::research_ensure,
            commands::life::research_update,
            commands::life::research_save_to_doc,
            commands::life::study_list_subjects,
            commands::life::study_upsert_subject,
            commands::life::study_delete_subject,
            commands::life::study_list_topics,
            commands::life::study_upsert_topic,
            commands::life::study_delete_topic,
            commands::life::study_list_assignments,
            commands::life::study_upsert_assignment,
            commands::life::study_delete_assignment,
            commands::life::study_list_sessions,
            commands::life::study_log_session,
            commands::life::fitness_overview,
            commands::life::fitness_list_food,
            commands::life::fitness_upsert_food,
            commands::life::fitness_delete_food,
            commands::life::fitness_list_fridge,
            commands::life::fitness_upsert_fridge,
            commands::life::fitness_delete_fridge,
            commands::life::fitness_list_recipes,
            commands::life::fitness_suggest_meals,
            commands::life::fitness_list_weight,
            commands::life::fitness_upsert_weight,
            commands::life::fitness_delete_weight,
            commands::life::fitness_list_climbs,
            commands::life::fitness_upsert_climb,
            commands::life::fitness_delete_climb,
            commands::life::fitness_list_workouts,
            commands::life::fitness_save_workout,
            commands::life::fitness_delete_workout,
            commands::life::fitness_list_prs,
            commands::life::money_list,
            commands::life::money_upsert,
            commands::life::money_delete,
            commands::life::money_summary,
            commands::life::money_analyze,
            commands::life::money_list_pots,
            commands::life::money_upsert_pot,
            commands::life::money_delete_pot,
            commands::life::socials_profile,
            commands::life::socials_update_profile,
            commands::life::socials_list_threads,
            commands::life::socials_update_thread,
            commands::life::socials_list_projects,
            commands::life::socials_upsert_project,
            commands::life::socials_delete_project,
            commands::life::socials_list_ideas,
            commands::life::socials_upsert_idea,
            commands::life::socials_delete_idea,
            commands::life::socials_list_drafts,
            commands::life::socials_upsert_draft,
            commands::life::socials_delete_draft,
            commands::life::socials_get_plan,
            commands::life::socials_start_review,
            commands::life::socials_generate_posts,
            commands::life::socials_archive_post,
            commands::life::socials_update_post,
            commands::life::socials_commit_approved,
            commands::life::socials_mark_published,
            commands::life::socials_published,
            commands::life::socials_get_by_event,
            commands::life::life_dashboard_snapshot,
            commands::life::life_dashboard_week,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                if let Some(pm) = app.try_state::<Arc<ProcessManager>>() {
                    pm.stop_owned_services();
                }
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
