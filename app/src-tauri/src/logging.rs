use std::io;
use std::path::PathBuf;

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_logging(logs_dir: &PathBuf, level: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("buddy={level},info")));

    let stdout_layer = fmt::layer().with_writer(io::stdout);

    if std::fs::create_dir_all(logs_dir).is_ok() {
        let file_appender = tracing_appender::rolling::daily(logs_dir, "buddy.log");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
        std::mem::forget(_guard);

        tracing_subscriber::registry()
            .with(filter)
            .with(stdout_layer)
            .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(stdout_layer)
            .init();
    }
}
