//! 同时写 stderr 与本地滚动日志文件；`RUST_LOG` 可覆盖默认 info 级别。

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub fn init() -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(true);

    let Some(log_dir) = log_dir() else {
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .try_init();
        return None;
    };
    if std::fs::create_dir_all(&log_dir).is_err() {
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .try_init();
        return None;
    }

    let appender = tracing_appender::rolling::daily(log_dir, "tourism-media.log");
    let (file_writer, guard) = tracing_appender::non_blocking(appender);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_target(true);
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(file_layer)
        .try_init();
    Some(guard)
}

fn log_dir() -> Option<std::path::PathBuf> {
    let dirs =
        directories::ProjectDirs::from("com", "TourismMediaOrganizer", "tourism-media-organizer")?;
    Some(dirs.data_local_dir().join("logs"))
}
