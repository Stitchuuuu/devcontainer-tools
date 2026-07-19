use tracing_subscriber::{fmt, EnvFilter};

/// Initialize the stderr tracing subscriber.
///
/// Precedence (highest wins):
/// 1. `NOTIF_LOG_LEVEL=trace|debug|info|warn|error` (or full env-filter spec).
///    Ignored if empty / whitespace / unparseable.
/// 2. `--quiet` flag or `NOTIF_QUIET=1` env → `warn`.
/// 3. `--verbose` flag → `debug`.
/// 4. Default → `info`.
///
/// Idempotent — safe to call more than once (subsequent calls no-op).
pub fn init_logging(verbose: bool, quiet: bool) {
    let explicit = std::env::var("NOTIF_LOG_LEVEL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let filter = match explicit {
        Some(spec) => {
            EnvFilter::try_new(&spec).unwrap_or_else(|_| default_filter(verbose, quiet))
        }
        None => default_filter(verbose, quiet),
    };

    let _ = fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .with_target(true)
        .try_init();
}

fn default_filter(verbose: bool, quiet: bool) -> EnvFilter {
    let level = if quiet {
        "warn"
    } else if verbose {
        "debug"
    } else {
        "info"
    };
    EnvFilter::new(level)
}
