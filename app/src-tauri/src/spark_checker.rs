use std::sync::Arc;
use std::time::Duration;

use buddy_database::{SPARK_NUDGE_COOLDOWN_MS, SPARK_STALE_AGE_MS};
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use tracing::{info, warn};

use crate::state::AppState;

const MAX_NOTIFICATIONS_PER_RUN: usize = 5;

pub fn spawn_spark_checker(app: AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        request_notification_permission(&app);
        run_spark_check(&app, &state).await;

        let mut interval = tokio::time::interval(Duration::from_secs(24 * 60 * 60));
        interval.tick().await;
        loop {
            interval.tick().await;
            run_spark_check(&app, &state).await;
        }
    });
}

fn request_notification_permission(app: &AppHandle) {
    let _ = app.notification().request_permission();
}

async fn run_spark_check(app: &AppHandle, state: &AppState) {
    let sparks = match state
        .db
        .get_stale_sparks(SPARK_STALE_AGE_MS, SPARK_NUDGE_COOLDOWN_MS)
    {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "failed to query stale sparks");
            return;
        }
    };

    let count = sparks.len() as i64;
    let _ = app.emit("sparks-stale", count);

    if sparks.is_empty() {
        return;
    }

    info!(count = sparks.len(), "stale sparks found");

    let batch: Vec<_> = sparks.iter().take(MAX_NOTIFICATIONS_PER_RUN).collect();
    let mut nudged_ids = Vec::new();

    for spark in &batch {
        let tags = spark.tags.join(", ");
        let preview: String = spark.content.chars().take(80).collect();
        let body = if spark.content.len() > 80 {
            format!("{preview}…")
        } else {
            preview
        };

        if let Err(e) = app
            .notification()
            .builder()
            .title("Spark needs you")
            .body(format!("{tags}: {body}"))
            .show()
        {
            warn!(error = %e, "notification failed");
        } else {
            nudged_ids.push(spark.id.clone());
        }
    }

    if !nudged_ids.is_empty() {
        if let Err(e) = state.db.mark_sparks_nudged(&nudged_ids) {
            warn!(error = %e, "failed to mark sparks nudged");
        }
    }

    let remaining = state
        .db
        .count_stale_sparks(SPARK_STALE_AGE_MS, SPARK_NUDGE_COOLDOWN_MS)
        .unwrap_or(count);
    let _ = app.emit("sparks-stale", remaining);
}
