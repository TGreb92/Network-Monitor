//! # Preset Packs
//!
//! Named collections of target presets. Built-in packs are always available;
//! custom packs are persisted to a TOML file next to the executable.
//! Supports JSON export/import for sharing packs between users.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::core::config::TargetPreset;
use crate::core::server_check::TestMode;

const PACKS_FILENAME: &str = "network-monitor-packs.toml";

/// A saved preset pack (user-created or built-in)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedPresetPack {
    pub name: String,
    pub presets: Vec<TargetPreset>,
}

/// Top-level packs config stored on disk
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PacksConfig {
    /// Name of the currently active pack (empty = use presets from main config)
    #[serde(default)]
    pub active_pack: String,
    /// User-created preset packs
    #[serde(default)]
    pub custom_packs: Vec<SavedPresetPack>,
}

impl Default for PacksConfig {
    fn default() -> Self {
        Self {
            active_pack: String::new(),
            custom_packs: Vec::new(),
        }
    }
}

/// Built-in preset packs (not editable, always available)
pub fn builtin_packs() -> Vec<SavedPresetPack> {
    vec![
        SavedPresetPack {
            name: "DNS Basics".into(),
            presets: dns_presets(),
        },
        SavedPresetPack {
            name: "Gaming + DNS".into(),
            presets: gaming_presets(),
        },
    ]
}

fn dns_presets() -> Vec<TargetPreset> {
    vec![
        TargetPreset { name: "Google DNS".into(), host: "8.8.8.8".into(), mode: TestMode::Icmp, category: "DNS".into() },
        TargetPreset { name: "Cloudflare DNS".into(), host: "1.1.1.1".into(), mode: TestMode::Icmp, category: "DNS".into() },
        TargetPreset { name: "Quad9 DNS".into(), host: "9.9.9.9".into(), mode: TestMode::Icmp, category: "DNS".into() },
        TargetPreset { name: "OpenDNS".into(), host: "208.67.222.222".into(), mode: TestMode::Icmp, category: "DNS".into() },
    ]
}

fn gaming_presets() -> Vec<TargetPreset> {
    vec![
        // DNS servers (ICMP — always respond to ping)
        TargetPreset { name: "Google DNS".into(), host: "8.8.8.8".into(), mode: TestMode::Icmp, category: "DNS".into() },
        TargetPreset { name: "Cloudflare DNS".into(), host: "1.1.1.1".into(), mode: TestMode::Icmp, category: "DNS".into() },
        TargetPreset { name: "Quad9 DNS".into(), host: "9.9.9.9".into(), mode: TestMode::Icmp, category: "DNS".into() },
        TargetPreset { name: "OpenDNS".into(), host: "208.67.222.222".into(), mode: TestMode::Icmp, category: "DNS".into() },
        // Riot Games (TCP — game servers block ICMP)
        TargetPreset { name: "Riot - NA API".into(), host: "na1.api.riotgames.com".into(), mode: TestMode::Tcp { port: 443 }, category: "Riot Games".into() },
        TargetPreset { name: "Riot - EU API".into(), host: "euw1.api.riotgames.com".into(), mode: TestMode::Tcp { port: 443 }, category: "Riot Games".into() },
        TargetPreset { name: "Riot - Auth".into(), host: "authenticate.riotgames.com".into(), mode: TestMode::Tcp { port: 443 }, category: "Riot Games".into() },
        // Valve / Steam (mix of ICMP + TCP)
        TargetPreset { name: "Valve - US East".into(), host: "192.223.24.53".into(), mode: TestMode::Icmp, category: "Valve/Steam".into() },
        TargetPreset { name: "Valve - EU".into(), host: "146.66.152.12".into(), mode: TestMode::Icmp, category: "Valve/Steam".into() },
        TargetPreset { name: "Valve - Asia".into(), host: "103.10.125.1".into(), mode: TestMode::Icmp, category: "Valve/Steam".into() },
        TargetPreset { name: "Steam Store".into(), host: "store.steampowered.com".into(), mode: TestMode::Tcp { port: 443 }, category: "Valve/Steam".into() },
        // Blizzard / Battle.net
        TargetPreset { name: "Blizzard - US".into(), host: "137.221.106.100".into(), mode: TestMode::Icmp, category: "Blizzard".into() },
        TargetPreset { name: "Blizzard - EU".into(), host: "185.60.112.157".into(), mode: TestMode::Icmp, category: "Blizzard".into() },
        TargetPreset { name: "Battle.net US".into(), host: "us.battle.net".into(), mode: TestMode::Tcp { port: 443 }, category: "Blizzard".into() },
        TargetPreset { name: "Battle.net EU".into(), host: "eu.battle.net".into(), mode: TestMode::Tcp { port: 443 }, category: "Blizzard".into() },
        TargetPreset { name: "Battle.net Asia".into(), host: "kr.battle.net".into(), mode: TestMode::Tcp { port: 443 }, category: "Blizzard".into() },
        // Epic Games / Fortnite
        TargetPreset { name: "Fortnite Services".into(), host: "fortnite-public-service-prod11.ol.epicgames.com".into(), mode: TestMode::Tcp { port: 443 }, category: "Epic Games".into() },
        TargetPreset { name: "Epic Store".into(), host: "epicgames.com".into(), mode: TestMode::Tcp { port: 443 }, category: "Epic Games".into() },
        // Call of Duty / Activision
        TargetPreset { name: "Activision".into(), host: "www.activision.com".into(), mode: TestMode::Tcp { port: 443 }, category: "Activision".into() },
        TargetPreset { name: "Call of Duty".into(), host: "www.callofduty.com".into(), mode: TestMode::Tcp { port: 443 }, category: "Activision".into() },
        // EA / Battlefield
        TargetPreset { name: "EA - EU".into(), host: "159.153.72.252".into(), mode: TestMode::Icmp, category: "EA".into() },
        TargetPreset { name: "EA.com".into(), host: "www.ea.com".into(), mode: TestMode::Tcp { port: 443 }, category: "EA".into() },
        TargetPreset { name: "EA Accounts".into(), host: "accounts.ea.com".into(), mode: TestMode::Tcp { port: 443 }, category: "EA".into() },
        // Platform services
        TargetPreset { name: "Xbox Live".into(), host: "xbox.com".into(), mode: TestMode::Tcp { port: 443 }, category: "Platform".into() },
        TargetPreset { name: "PlayStation".into(), host: "store.playstation.com".into(), mode: TestMode::Tcp { port: 443 }, category: "Platform".into() },
    ]
}

/// Get the path to the packs file (next to the executable)
fn packs_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(PACKS_FILENAME)))
        .unwrap_or_else(|| PathBuf::from(PACKS_FILENAME))
}

/// Load packs config from disk. Returns defaults if file doesn't exist.
pub fn load() -> PacksConfig {
    let path = packs_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
        Err(_) => PacksConfig::default(),
    }
}

/// Save packs config to disk.
pub fn save(config: &PacksConfig) -> std::io::Result<()> {
    let path = packs_path();
    let contents = toml::to_string_pretty(config)
        .map_err(std::io::Error::other)?;
    std::fs::write(&path, contents)
}

/// Export a single pack as pretty-printed JSON.
pub fn export_pack_json(pack: &SavedPresetPack, path: &std::path::Path) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(pack)
        .map_err(std::io::Error::other)?;
    std::fs::write(path, json)
}

/// Import a pack from a JSON file.
pub fn import_pack_json(path: &std::path::Path) -> Result<SavedPresetPack, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    serde_json::from_str(&contents)
        .map_err(|e| format!("Invalid pack JSON: {}", e))
}

