use std::path::PathBuf;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

pub fn log_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".tdl")
        .join("logs")
}

/// Parse log level string to tracing::Level, defaulting to INFO.
pub fn parse_level(s: &str) -> Level {
    match s.to_lowercase().as_str() {
        "error" => Level::ERROR,
        "warn"  => Level::WARN,
        "debug" => Level::DEBUG,
        "trace" => Level::TRACE,
        _       => Level::INFO,
    }
}

/// Initialize logging with file output and optional stderr output.
///
/// - File: `~/.tdl/logs/tdl.log.<date>` (daily rolling, 7-day retention)
/// - Stderr: only when `stderr` is true (CLI mode)
/// - Level: from `log_level` string, overridden by `RUST_LOG` env var
///
/// Returns a WorkerGuard that must stay alive for the process lifetime.
pub fn init_with_level(log_level: &str, stderr: bool) -> WorkerGuard {
    let dir = log_dir();
    std::fs::create_dir_all(&dir).ok();
    purge_old_logs(&dir, 7);

    // Produces tdl.2026-04-28.log (prefix.date.suffix) via the builder API.
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("tdl")
        .filename_suffix("log")
        .build(&dir)
        .expect("failed to create log appender");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let level = parse_level(log_level);

    // Prefer RUST_LOG env; fall back to configured level.
    let env_filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_env("RUST_LOG")
    } else {
        EnvFilter::new(format!("tdl={level},warn"))
    };

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .boxed();

    if stderr {
        let stderr_layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(true)
            .with_target(false)
            .with_level(true)
            .boxed();

        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .with(stderr_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .init();
    }

    guard
}

/// Convenience wrapper used by main() before settings are loaded.
pub fn init() -> WorkerGuard {
    // Try to read log_level from settings; fall back to INFO.
    let level = crate::config::settings::Settings::load()
        .map(|s| s.log_level)
        .unwrap_or_else(|_| "info".to_string());
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    init_with_level(&level, is_tty)
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
        if let Ok(meta) = entry.metadata()
            && let Ok(modified) = meta.modified()
            && modified < cutoff
        {
            std::fs::remove_file(&path).ok();
        }
    }
}
