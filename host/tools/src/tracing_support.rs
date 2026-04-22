//! Shared tracing initialization for the operational tooling binary.

use tracing_subscriber::EnvFilter;

use crate::cli::LogLevel;

/// Initialize tracing once for the current process.
pub fn init(level: LogLevel, no_color: bool) {
    let filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else {
        EnvFilter::new(level.as_str())
    };

    let _ = tracing_subscriber::fmt()
        .without_time()
        .with_target(false)
        .with_ansi(!no_color)
        .with_env_filter(filter)
        .try_init();
}
