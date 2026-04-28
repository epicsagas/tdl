use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

pub fn log_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".tdl")
        .join("logs")
}

/// Initialize daily rolling log to ~/.tdl/logs/tdl.YYYY-MM-DD.log.
/// Returns a WorkerGuard that must be kept alive for the duration of the process.
pub fn init() -> WorkerGuard {
    let dir = log_dir();
    std::fs::create_dir_all(&dir).ok();

    purge_old_logs(&dir, 7);

    let file_appender = tracing_appender::rolling::daily(&dir, "tdl.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(false)
        .init();

    guard
}

fn purge_old_logs(dir: &PathBuf, keep_days: u64) {
    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(keep_days * 24 * 60 * 60);

    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("log") {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                if modified < cutoff {
                    std::fs::remove_file(&path).ok();
                }
            }
        }
    }
}
