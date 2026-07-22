use std::sync::Arc;
use std::time::Duration;

use buddy_database::chrono_now;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use tracing::{info, warn};

use crate::state::AppState;

const POLL_INTERVAL_SECS: u64 = 60;
const MAX_NOTIFICATIONS_PER_TICK: usize = 10;

pub fn spawn_calendar_reminder_checker(app: AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        // Small delay so the UI can finish loading before the first poll.
        tokio::time::sleep(Duration::from_secs(3)).await;
        loop {
            run_reminder_check(&app, &state).await;
            tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
        }
    });
}

async fn run_reminder_check(app: &AppHandle, state: &AppState) {
    let now = chrono_now();
    let due = match state.calendar.list_due_reminders(now).await {
        Ok(d) => d,
        Err(e) => {
            warn!(error = %e, "failed to list due calendar reminders");
            return;
        }
    };

    if due.is_empty() {
        return;
    }

    info!(count = due.len(), "calendar reminders due");
    let desktop = state.calendar.notifications_enabled();

    for delivery in due.iter().take(MAX_NOTIFICATIONS_PER_TICK) {
        if desktop {
            if let Err(e) = app
                .notification()
                .builder()
                .title("Calendar reminder")
                .body(format!(
                    "{} ({} min before)",
                    delivery.event_title, delivery.reminder_minutes
                ))
                .show()
            {
                warn!(error = %e, "calendar desktop notification failed");
            }
        }

        if let Err(e) = state.calendar.mark_reminder_sent(&delivery.id).await {
            warn!(error = %e, "failed to mark reminder sent");
            continue;
        }

        let _ = app.emit("calendar-reminder", delivery);
    }

    if let Ok(count) = state.calendar.notification_count().await {
        let _ = app.emit("calendar-notification-count", count);
    }
}
