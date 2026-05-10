//! # Config Persistence
//!
//! Loads and saves user configuration to a TOML file next to the executable.
//! Falls back to defaults if the file doesn't exist or can't be parsed.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const CONFIG_FILENAME: &str = "network-monitor.toml";

/// A named target preset (e.g. "Google DNS" → "8.8.8.8")
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TargetPreset {
    pub name: String,
    pub host: String,
}

/// Serializable config that gets saved to disk
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedConfig {
    /// Index of the currently selected preset
    pub selected_preset: usize,
    pub timeout_ms: u32,
    pub interval_secs: u64,
    pub ping_interval_ms: u64,
    pub gateway_enabled: bool,
    pub auto_detect_gateway: bool,
    pub duration_mins: u64,
    pub presets: Vec<TargetPreset>,
}

impl Default for SavedConfig {
    fn default() -> Self {
        Self {
            selected_preset: 0,
            timeout_ms: 2000,
            interval_secs: 60,
            ping_interval_ms: 1000,
            gateway_enabled: false,
            auto_detect_gateway: true,
            duration_mins: 0,
            presets: default_presets(),
        }
    }
}

/// Built-in default presets
pub fn default_presets() -> Vec<TargetPreset> {
    vec![
        TargetPreset { name: "Google DNS".into(), host: "8.8.8.8".into() },
        TargetPreset { name: "Cloudflare DNS".into(), host: "1.1.1.1".into() },
        TargetPreset { name: "Quad9 DNS".into(), host: "9.9.9.9".into() },
        TargetPreset { name: "OpenDNS".into(), host: "208.67.222.222".into() },
    ]
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
        Ok(contents) => {
            let mut config: SavedConfig = toml::from_str(&contents).unwrap_or_default();
            if config.presets.is_empty() {
                config.presets = default_presets();
            }
            if config.selected_preset >= config.presets.len() {
                config.selected_preset = 0;
            }
            config
        }
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
