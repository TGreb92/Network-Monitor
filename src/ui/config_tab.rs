//! # Config Tab — Settings with TOML persistence
//!
//! Target presets, ping parameters, gateway settings, and duration.
//! Saves to and loads from network-monitor.toml.

use eframe::egui;
use std::time::Instant;

use crate::core::config::{self, SavedConfig};
use crate::core::state::{PingConfig, SharedState, PRESETS};

/// Config tab local state (buffered until Apply is clicked)
pub struct ConfigState {
    pub target: String,
    pub timeout: u32,
    pub interval: u64,
    pub ping_freq: u64,
    pub gateway_enabled: bool,
    pub auto_detect_gateway: bool,
    pub duration_mins: u64,
    pub selected_preset: usize,
    pub status: Option<(String, Instant)>,
}

impl ConfigState {
    pub fn from_saved(saved: &SavedConfig) -> Self {
        Self {
            target: saved.target.clone(),
            timeout: saved.timeout_ms,
            interval: saved.interval_secs,
            ping_freq: saved.ping_interval_ms,
            gateway_enabled: saved.gateway_enabled,
            auto_detect_gateway: saved.auto_detect_gateway,
            duration_mins: saved.duration_mins,
            selected_preset: 0,
            status: None,
        }
    }
}

/// Render the Config tab contents
pub fn render(ui: &mut egui::Ui, state: &SharedState, cfg: &mut ConfigState) {
    ui.heading("⚙ Configuration");
    ui.add_space(8.0);

    render_presets(ui, cfg);
    ui.add_space(8.0);
    render_fields(ui, cfg);
    ui.add_space(12.0);
    render_buttons(ui, state, cfg);
    render_status(ui, cfg);
}

fn render_presets(ui: &mut egui::Ui, cfg: &mut ConfigState) {
    ui.label("Quick presets:");
    egui::ComboBox::from_id_salt("preset_combo")
        .selected_text(format!(
            "{} ({})",
            PRESETS[cfg.selected_preset].0,
            PRESETS[cfg.selected_preset].1
        ))
        .show_ui(ui, |ui| {
            for (idx, (ip, name)) in PRESETS.iter().enumerate() {
                if ui.selectable_value(&mut cfg.selected_preset, idx, format!("{ip} ({name})")).clicked() {
                    cfg.target = ip.to_string();
                }
            }
        });
}

fn render_fields(ui: &mut egui::Ui, cfg: &mut ConfigState) {
    ui.label("Target host:");
    ui.text_edit_singleline(&mut cfg.target);
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

fn render_buttons(ui: &mut egui::Ui, state: &SharedState, cfg: &mut ConfigState) {
    ui.horizontal(|ui| {
        if ui.button("✅ Apply & Save").clicked() {
            apply_and_save(state, cfg);
        }
        if ui.button("🔄 Reset to Defaults").clicked() {
            let defaults = SavedConfig::default();
            *cfg = ConfigState::from_saved(&defaults);
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

fn apply_and_save(state: &SharedState, cfg: &mut ConfigState) {
    // Apply to shared state
    let mut shared = state.lock().unwrap_or_else(|err| err.into_inner());
    shared.config = PingConfig {
        target: cfg.target.clone(),
        timeout_ms: cfg.timeout,
        interval_secs: cfg.interval,
        ping_interval_ms: cfg.ping_freq.max(100),
        duration_secs: cfg.duration_mins * 60,
    };
    shared.gateway_enabled = cfg.gateway_enabled;
    shared.config_changed = true;
    drop(shared);

    // Save to disk
    let saved = SavedConfig {
        target: cfg.target.clone(),
        timeout_ms: cfg.timeout,
        interval_secs: cfg.interval,
        ping_interval_ms: cfg.ping_freq.max(100),
        gateway_enabled: cfg.gateway_enabled,
        auto_detect_gateway: cfg.auto_detect_gateway,
        duration_mins: cfg.duration_mins,
    };
    match config::save(&saved) {
        Ok(()) => cfg.status = Some(("✅ Config saved".into(), Instant::now())),
        Err(err) => cfg.status = Some((format!("❌ Save failed: {}", err), Instant::now())),
    }
}
