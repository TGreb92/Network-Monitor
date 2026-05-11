//! # Config Tab — Settings with TOML persistence
//!
//! Ping parameters, gateway settings, and duration.
//! Preset management is in `presets.rs`.

use eframe::egui;
use std::time::Instant;

use crate::core::config::{self, SavedConfig, default_presets};
use crate::core::state::{PingConfig, SharedState};
use crate::ui::presets::{self, PresetEditorState};
use crate::ui::sidebar::SidebarState;

/// Config tab local state
pub struct ConfigState {
    pub timeout: u32,
    pub interval: u64,
    pub ping_freq: u64,
    pub gateway_enabled: bool,
    pub auto_detect_gateway: bool,
    pub duration_mins: u64,
    pub status: Option<(String, Instant)>,
    pub preset_editor: PresetEditorState,
}

impl ConfigState {
    pub fn from_saved(saved: &SavedConfig) -> Self {
        Self {
            timeout: saved.timeout_ms,
            interval: saved.interval_secs,
            ping_freq: saved.ping_interval_ms,
            gateway_enabled: saved.gateway_enabled,
            auto_detect_gateway: saved.auto_detect_gateway,
            duration_mins: saved.duration_mins,
            status: None,
            preset_editor: PresetEditorState::new(),
        }
    }
}

/// Render the Config tab contents
pub fn render(ui: &mut egui::Ui, state: &SharedState, cfg: &mut ConfigState, sidebar: &mut SidebarState) {
    ui.heading("⚙ Configuration");
    ui.add_space(8.0);

    presets::render(ui, &mut cfg.preset_editor, sidebar);
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);
    render_fields(ui, cfg);
    ui.add_space(12.0);
    render_buttons(ui, state, cfg, sidebar);
    render_status(ui, cfg);
}

fn render_fields(ui: &mut egui::Ui, cfg: &mut ConfigState) {
    ui.heading("🔧 Ping Settings");
    ui.add_space(4.0);

    ui.label("Timeout (ms):");
    ui.add(egui::DragValue::new(&mut cfg.timeout).range(100..=30000).speed(50).suffix(" ms"));
    ui.add_space(4.0);

    ui.label("Report interval (s):");
    ui.add(egui::DragValue::new(&mut cfg.interval).range(5..=3600).speed(1).suffix(" s"));
    ui.add_space(4.0);

    ui.label("Ping frequency (ms):");
    ui.add(egui::DragValue::new(&mut cfg.ping_freq).range(100..=10000).speed(50).suffix(" ms"));
    ui.add_space(4.0);

    ui.checkbox(&mut cfg.gateway_enabled, "Enable gateway monitoring");
    ui.checkbox(&mut cfg.auto_detect_gateway, "Auto-detect gateway on startup");
    ui.add_space(8.0);

    ui.label("Test duration (minutes, 0 = unlimited):");
    ui.add(egui::DragValue::new(&mut cfg.duration_mins).range(0..=1440).speed(1).suffix(" min"));
}

fn render_buttons(ui: &mut egui::Ui, state: &SharedState, cfg: &mut ConfigState, sidebar: &mut SidebarState) {
    ui.horizontal(|ui| {
        if ui.button("✅ Apply & Save").clicked() {
            apply_and_save(state, cfg, sidebar);
        }
        if ui.button("🔄 Reset to Defaults").clicked() {
            let defaults = SavedConfig::default();
            *cfg = ConfigState::from_saved(&defaults);
            sidebar.presets = default_presets();
            sidebar.selected_preset = 0;
        }
    });
}

fn render_status(ui: &mut egui::Ui, cfg: &ConfigState) {
    if let Some((text, when)) = &cfg.status {
        if when.elapsed().as_secs() < 5 {
            ui.add_space(8.0);
            ui.colored_label(egui::Color32::from_rgb(150, 200, 150), text);
        }
    }
}

fn apply_and_save(state: &SharedState, cfg: &mut ConfigState, sidebar: &SidebarState) {
    let target = sidebar.selected_host();
    let mut shared = state.lock().unwrap_or_else(|err| err.into_inner());
    shared.config = PingConfig {
        target,
        timeout_ms: cfg.timeout,
        interval_secs: cfg.interval,
        ping_interval_ms: cfg.ping_freq.max(100),
        duration_secs: cfg.duration_mins * 60,
    };
    shared.gateway.enabled = cfg.gateway_enabled;
    shared.config_changed = true;
    drop(shared);

    let saved = SavedConfig {
        selected_preset: sidebar.selected_preset,
        timeout_ms: cfg.timeout,
        interval_secs: cfg.interval,
        ping_interval_ms: cfg.ping_freq.max(100),
        gateway_enabled: cfg.gateway_enabled,
        auto_detect_gateway: cfg.auto_detect_gateway,
        duration_mins: cfg.duration_mins,
        presets: sidebar.presets.clone(),
    };
    match config::save(&saved) {
        Ok(()) => cfg.status = Some(("✅ Config saved".into(), Instant::now())),
        Err(err) => cfg.status = Some((format!("❌ Save failed: {}", err), Instant::now())),
    }
}
