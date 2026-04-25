//! Logging initialization for mdink.
//!
//! Implements a two-phase logger to respect the TUI constraint: after the
//! terminal alternate screen is active, log output must not go to stderr
//! (which corrupts the TUI display).
//!
//! **Phase A** (before `ratatui::init()`): logs go to stderr or a file.
//! **Phase B** (after `ratatui::init()`): silences stderr via `set_max_level(Off)`,
//! or does nothing if Phase A already writes to a file.
//!
//! Precedence for log level: CLI > `MDINK_LOG_LEVEL` env > config > `RUST_LOG` > default (Warn).
//! Precedence for log file: CLI > `MDINK_LOG_FILE` env > config > auto-detected cache dir.

use std::path::PathBuf;

use env_logger::Builder;
use log::LevelFilter;

/// Resolved logging configuration.
pub struct LogConfig {
    /// Maximum log level to emit.
    pub level: LevelFilter,
    /// File path for log output. `None` means no file output.
    pub file_path: Option<PathBuf>,
    /// Whether the user explicitly requested logging (via CLI, env, or config).
    /// When false and no file is set, Phase B suppresses all output.
    pub explicit: bool,
}

/// Resolves log configuration from CLI, environment, and config file.
///
/// Precedence: CLI > env var > config > RUST_LOG > default (Warn).
pub fn resolve_log_config(
    cli_log_level: Option<&str>,
    cli_log_file: Option<&str>,
    config_log_level: Option<&str>,
    config_log_file: Option<&str>,
) -> LogConfig {
    let level = resolve_level(cli_log_level, config_log_level);
    let (file_path, explicit_file) = resolve_file(cli_log_file, config_log_file);
    let explicit = cli_log_level.is_some()
        || std::env::var("MDINK_LOG_LEVEL").is_ok()
        || config_log_level.is_some()
        || std::env::var("RUST_LOG").is_ok();

    LogConfig {
        level,
        file_path,
        explicit: explicit || explicit_file,
    }
}

fn resolve_level(cli: Option<&str>, config: Option<&str>) -> LevelFilter {
    // CLI flag takes highest precedence.
    if let Some(level_str) = cli {
        return parse_level(level_str);
    }
    // Then env var.
    if let Ok(level_str) = std::env::var("MDINK_LOG_LEVEL") {
        return parse_level(&level_str);
    }
    // Then config file.
    if let Some(level_str) = config {
        return parse_level(level_str);
    }
    // Then RUST_LOG (standard Rust convention).
    if std::env::var("RUST_LOG").is_ok() {
        // env_logger will parse RUST_LOG itself; we just need to not override it.
        // Return Off here as a signal to not set a max level filter — let
        // env_logger handle RUST_LOG parsing.
        return LevelFilter::Off;
    }
    // Default: Warn.
    LevelFilter::Warn
}

fn parse_level(s: &str) -> LevelFilter {
    match s.to_ascii_lowercase().as_str() {
        "off" => LevelFilter::Off,
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Warn,
    }
}

fn resolve_file(cli: Option<&str>, config: Option<&str>) -> (Option<PathBuf>, bool) {
    if let Some(path) = cli {
        return (Some(PathBuf::from(path)), true);
    }
    if let Ok(path) = std::env::var("MDINK_LOG_FILE") {
        return (Some(PathBuf::from(path)), true);
    }
    if let Some(path) = config {
        return (Some(PathBuf::from(path)), true);
    }
    (None, false)
}

/// Returns the default log file path in the platform cache directory.
fn default_log_file() -> Option<PathBuf> {
    let cache_dir = dirs::cache_dir()?;
    Some(cache_dir.join("mdink").join("mdink.log"))
}

/// Initializes Phase A logger.
///
/// Call this before `ratatui::init()`.
///
/// Output routing:
/// - If a log file is specified (CLI/env/config): writes to file.
/// - If explicitly requested but no file: writes to default cache dir.
/// - Otherwise: writes to stderr (for pre-TUI fatal error visibility).
pub fn init_phase_a(config: &LogConfig) {
    let mut builder = Builder::new();

    if std::env::var("RUST_LOG").is_ok() && !config.explicit {
        // Let RUST_LOG control filtering; don't override.
        builder.parse_default_env();
    } else {
        builder.filter_level(config.level);
    }

    // Format: timestamp level [module_path] message
    builder.format_timestamp_secs();
    builder.format_target(true);

    // Determine output target:
    // 1. Explicit file → write to file
    // 2. Explicit but no file → write to default cache dir
    // 3. Not explicit → write to stderr (for pre-TUI fatal errors)
    let resolved_file = if let Some(ref path) = config.file_path {
        Some(path.clone())
    } else if config.explicit {
        default_log_file()
    } else {
        None
    };

    if let Some(path) = resolved_file {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(file) = std::fs::File::create(&path) {
            builder.target(env_logger::Target::Pipe(Box::new(file)));
        }
        // If file creation fails, falls through to stderr (harmless during Phase A).
    }

    let _ = builder.try_init();
}

/// Silences log output for Phase B (after TUI init).
///
/// The `log` crate only allows setting the global logger ONCE, so Phase B
/// cannot replace the Phase A logger. Instead, it uses `set_max_level()`
/// which is always callable and takes immediate effect across all threads.
///
/// - If Phase A already writes to a file: nothing to do (file is safe for TUI).
/// - Otherwise: sets max level to `Off` to prevent any stderr output.
pub fn init_phase_b(config: &LogConfig) {
    // If Phase A already set up a file logger, it's safe for TUI — do nothing.
    if config.file_path.is_some() || config.explicit {
        return;
    }

    // No file, not explicit — suppress all output to prevent stderr corruption.
    // set_max_level() is thread-safe and takes immediate effect.
    log::set_max_level(LevelFilter::Off);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_log_config_default() {
        // No CLI, no env, no config → Warn level, no file, not explicit.
        let lc = resolve_log_config(None, None, None, None);
        assert_eq!(lc.level, LevelFilter::Warn);
        assert!(lc.file_path.is_none());
        assert!(!lc.explicit);
    }

    #[test]
    fn test_resolve_log_config_cli_level() {
        let lc = resolve_log_config(Some("debug"), None, None, None);
        assert_eq!(lc.level, LevelFilter::Debug);
        assert!(lc.explicit);
    }

    #[test]
    fn test_resolve_log_config_cli_file() {
        let lc = resolve_log_config(None, Some("/tmp/test.log"), None, None);
        assert_eq!(lc.file_path, Some(PathBuf::from("/tmp/test.log")));
        assert!(lc.explicit);
    }

    #[test]
    fn test_resolve_log_config_cli_overrides_config() {
        let lc = resolve_log_config(Some("trace"), None, Some("info"), None);
        assert_eq!(lc.level, LevelFilter::Trace);
    }

    #[test]
    fn test_resolve_log_config_config_level() {
        let lc = resolve_log_config(None, None, Some("info"), None);
        assert_eq!(lc.level, LevelFilter::Info);
        assert!(lc.explicit);
    }

    #[test]
    fn test_parse_level_variants() {
        assert_eq!(parse_level("off"), LevelFilter::Off);
        assert_eq!(parse_level("error"), LevelFilter::Error);
        assert_eq!(parse_level("warn"), LevelFilter::Warn);
        assert_eq!(parse_level("info"), LevelFilter::Info);
        assert_eq!(parse_level("debug"), LevelFilter::Debug);
        assert_eq!(parse_level("trace"), LevelFilter::Trace);
        assert_eq!(parse_level("DEBUG"), LevelFilter::Debug);
        assert_eq!(parse_level("Info"), LevelFilter::Info);
    }

    #[test]
    fn test_parse_level_invalid() {
        assert_eq!(parse_level("verbose"), LevelFilter::Warn);
        assert_eq!(parse_level(""), LevelFilter::Warn);
    }

    #[test]
    fn test_resolve_file_cli_overrides_config() {
        let (path, explicit) = resolve_file(Some("/cli.log"), Some("/config.log"));
        assert_eq!(path, Some(PathBuf::from("/cli.log")));
        assert!(explicit);
    }

    #[test]
    fn test_resolve_file_config_only() {
        let (path, explicit) = resolve_file(None, Some("/config.log"));
        assert_eq!(path, Some(PathBuf::from("/config.log")));
        assert!(explicit);
    }

    #[test]
    fn test_resolve_file_none() {
        let (path, explicit) = resolve_file(None, None);
        assert!(path.is_none());
        assert!(!explicit);
    }
}
