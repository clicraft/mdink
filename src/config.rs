//! Config file loading for mdink.
//!
//! Reads `~/.config/mdink/config.json` (or the platform equivalent via `dirs`).
//! This module is loaded only by `main.rs` — never by `cli.rs` (xtask constraint).

use serde::Deserialize;

/// User configuration loaded from `~/.config/mdink/config.json`.
///
/// All fields are `Option` so that "not set" is distinguishable from an
/// explicit value. The precedence chain is: CLI flag > env var > config > default.
#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct Config {
    pub style: Option<String>,
    pub no_images: Option<bool>,
    pub no_color: Option<bool>,
}

/// Loads the config file, returning defaults on any failure.
///
/// - Missing file → `Config::default()` (silent).
/// - Parse error → warning to stderr + `Config::default()`.
pub fn load_config() -> Config {
    let Some(config_dir) = dirs::config_dir() else {
        return Config::default();
    };

    let path = config_dir.join("mdink").join("config.json");

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Config::default(),
        Err(e) => {
            eprintln!("warning: cannot read {}: {e}", path.display());
            return Config::default();
        }
    };

    match serde_json::from_str(&content) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("warning: invalid config {}: {e}", path.display());
            Config::default()
        }
    }
}
