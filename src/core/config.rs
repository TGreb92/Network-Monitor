//! # Config Persistence
//!
//! Loads and saves user configuration to a TOML file next to the executable.
//! Falls back to defaults if the file doesn't exist or can't be parsed.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const CONFIG_FILENAME: &str = "network-monitor.toml";

/// Whether a preset uses ICMP ping or TCP connect
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TestMode {
    Icmp,
    Tcp,
}

impl Default for TestMode {
    fn default() -> Self { Self::Icmp }
}

impl std::fmt::Display for TestMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Icmp => write!(f, "ICMP"),
            Self::Tcp => write!(f, "TCP"),
        }
    }
}

fn default_tcp_port() -> u16 { 443 }

/// A named target preset (e.g. "Google DNS" → "8.8.8.8")
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TargetPreset {
    pub name: String,
    pub host: String,
    #[serde(default)]
    pub mode: TestMode,
    #[serde(default = "default_tcp_port")]
    pub port: u16,
}

/// Serializable config that gets saved to disk
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedConfig {
    pub selected_preset: usize,
    pub timeout_ms: u32,
    pub interval_secs: u64,
    pub ping_interval_ms: u64,
    pub gateway_enabled: bool,
    pub auto_detect_gateway: bool,
    pub duration_mins: u64,
    pub presets: Vec<TargetPreset>,
    /// Custom export directory. Empty string = default (exe_dir/exports/).
    pub export_path: String,
    /// Auto-export flags - which formats to export when a test stops
    #[serde(default)]
    pub auto_export_csv: bool,
    #[serde(default)]
    pub auto_export_json: bool,
    #[serde(default)]
    pub auto_export_isp: bool,
    #[serde(default)]
    pub auto_export_log: bool,
    /// Show a popup notification when a loss event starts
    #[serde(default)]
    pub notify_on_loss: bool,
    /// Toast on gateway loss event
    #[serde(default)]
    pub notify_on_gw_loss: bool,
    /// Toast on elevated ping (>= elevated threshold)
    #[serde(default)]
    pub notify_on_elevated_ping: bool,
    /// Toast on high ping (>= high threshold)
    #[serde(default)]
    pub notify_on_high_ping: bool,
    /// Toast on critical ping (>= critical threshold)
    #[serde(default)]
    pub notify_on_critical_ping: bool,
    /// Elevated ping threshold in ms
    #[serde(default = "default_elevated_threshold")]
    pub threshold_elevated_ms: u32,
    /// High ping threshold in ms
    #[serde(default = "default_high_threshold")]
    pub threshold_high_ms: u32,
    /// Critical ping threshold in ms
    #[serde(default = "default_critical_threshold")]
    pub threshold_critical_ms: u32,
    /// Enable modem HTTP health check
    #[serde(default)]
    pub modem_health_enabled: bool,
    /// URL for modem health check (HTTP only)
    #[serde(default = "default_modem_health_url")]
    pub modem_health_url: String,
    /// Seconds between modem health checks
    #[serde(default = "default_modem_health_interval")]
    pub modem_health_interval_secs: u32,
    /// Window in minutes for modem struggle pattern detection
    #[serde(default = "default_modem_struggle_window")]
    pub modem_struggle_window_mins: u32,
}

fn default_elevated_threshold() -> u32 { 100 }
fn default_high_threshold() -> u32 { 200 }
fn default_critical_threshold() -> u32 { 500 }
fn default_modem_health_url() -> String { "http://192.168.0.1/?status_status".into() }
fn default_modem_health_interval() -> u32 { 15 }
fn default_modem_struggle_window() -> u32 { 5 }

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
            export_path: String::new(),
            auto_export_csv: false,
            auto_export_json: false,
            auto_export_isp: false,
            auto_export_log: false,
            notify_on_loss: false,
            notify_on_gw_loss: false,
            notify_on_elevated_ping: false,
            notify_on_high_ping: false,
            notify_on_critical_ping: false,
            threshold_elevated_ms: 100,
            threshold_high_ms: 200,
            threshold_critical_ms: 500,
            modem_health_enabled: false,
            modem_health_url: default_modem_health_url(),
            modem_health_interval_secs: 15,
            modem_struggle_window_mins: 5,
        }
    }
}

/// Built-in default presets
pub fn default_presets() -> Vec<TargetPreset> {
    vec![
        TargetPreset { name: "Google DNS".into(), host: "8.8.8.8".into(), mode: TestMode::Icmp, port: 443 },
        TargetPreset { name: "Cloudflare DNS".into(), host: "1.1.1.1".into(), mode: TestMode::Icmp, port: 443 },
        TargetPreset { name: "Quad9 DNS".into(), host: "9.9.9.9".into(), mode: TestMode::Icmp, port: 443 },
        TargetPreset { name: "OpenDNS".into(), host: "208.67.222.222".into(), mode: TestMode::Icmp, port: 443 },
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
        .map_err(std::io::Error::other)?;
    std::fs::write(&path, contents)
}
