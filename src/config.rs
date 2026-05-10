//! # Config Persistence
//!
//! Loads and saves user configuration to a TOML file next to the executable.
//! Falls back to defaults if the file doesn't exist or can't be parsed.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const CONFIG_FILENAME: &str = "network-monitor.toml";

/// Serializable config that gets saved to disk
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedConfig {
    pub target: String,
    pub timeout_ms: u32,
    pub interval_secs: u64,
    pub ping_interval_ms: u64,
    pub gateway_enabled: bool,
    pub auto_detect_gateway: bool,
    /// Test duration in minutes. 0 = unlimited (run until manually stopped).
    pub duration_mins: u64,
}

impl Default for SavedConfig {
    fn default() -> Self {
        Self {
            target: "8.8.8.8".to_string(),
            timeout_ms: 2000,
            interval_secs: 60,
            ping_interval_ms: 1000,
            gateway_enabled: false,
            auto_detect_gateway: true,
            duration_mins: 0,
        }
    }
}

/// Get the path to the config file (next to the executable)
fn config_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(CONFIG_FILENAME)))
        .unwrap_or_else(|| PathBuf::from(CONFIG_FILENAME))
}

/// Load config from disk. Returns defaults if file doesn't exist or is invalid.
pub fn load() -> SavedConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
        Err(_) => SavedConfig::default(),
    }
}

/// Save config to disk. Returns Ok(()) on success.
pub fn save(config: &SavedConfig) -> std::io::Result<()> {
    let path = config_path();
    let contents = toml::to_string_pretty(config)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
    std::fs::write(&path, contents)
}
