//! # Config Tab — Settings with TOML persistence
//!
//! Preset management (add/edit/delete), ping parameters, gateway settings.
//! Saves to and loads from network-monitor.toml.

use eframe::egui;
use std::time::Instant;

use crate::core::config::{self, SavedConfig, TargetPreset, default_presets};
use crate::core::state::{PingConfig, SharedState};
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
    // Preset editor fields
    pub edit_name: String,
    pub edit_host: String,
    pub editing_index: Option<usize>,
    pub show_add_form: bool,
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
            edit_name: String::new(),
            edit_host: String::new(),
            editing_index: None,
            show_add_form: false,
        }
    }
}

/// Render the Config tab contents
pub fn render(ui: &mut egui::Ui, state: &SharedState, cfg: &mut ConfigState, sidebar: &mut SidebarState) {
    ui.heading("⚙ Configuration");
    ui.add_space(8.0);

    render_preset_manager(ui, cfg, sidebar);
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);
    render_fields(ui, cfg);
    ui.add_space(12.0);
    render_buttons(ui, state, cfg, sidebar);
    render_status(ui, cfg);
}

fn render_preset_manager(ui: &mut egui::Ui, cfg: &mut ConfigState, sidebar: &mut SidebarState) {
    let header_id = ui.make_persistent_id("presets_collapsible");
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), header_id, false)
        .show_header(ui, |ui| {
            ui.heading("🎯 Target Presets");
            ui.label(format!("({})", sidebar.presets.len()));
        })
        .body(|ui| {
            render_preset_list(ui, cfg, sidebar);
            ui.add_space(4.0);
            render_preset_form(ui, cfg, sidebar);
        });
}

fn render_preset_list(ui: &mut egui::Ui, cfg: &mut ConfigState, sidebar: &mut SidebarState) {
    let mut delete_index: Option<usize> = None;
    let mut start_edit_index: Option<usize> = None;

    egui::ScrollArea::vertical()
        .id_salt("preset_list")
        .max_height(150.0)
        .show(ui, |ui| {
            egui::Grid::new("preset_grid")
                .striped(true)
                .min_col_width(20.0)
                .show(ui, |ui| {
                    for (idx, preset) in sidebar.presets.iter().enumerate() {
                        let is_selected = idx == sidebar.selected_preset;

                        if ui.selectable_label(is_selected, &preset.name).clicked() {
                            sidebar.selected_preset = idx;
                        }
                        ui.label(&preset.host);
                        if ui.small_button("✏").on_hover_text("Edit").clicked() {
                            start_edit_index = Some(idx);
                        }
                        if ui.small_button("🗑").on_hover_text("Delete").clicked() {
                            delete_index = Some(idx);
                        }
                        ui.end_row();
                    }
                });
        });

    if let Some(idx) = delete_index {
        if sidebar.presets.len() > 1 {
            sidebar.presets.remove(idx);
            if sidebar.selected_preset >= sidebar.presets.len() {
                sidebar.selected_preset = sidebar.presets.len() - 1;
            }
        }
    }

    if let Some(idx) = start_edit_index {
        cfg.edit_name = sidebar.presets[idx].name.clone();
        cfg.edit_host = sidebar.presets[idx].host.clone();
        cfg.editing_index = Some(idx);
    }
}

fn render_preset_form(ui: &mut egui::Ui, cfg: &mut ConfigState, sidebar: &mut SidebarState) {
    let is_editing = cfg.editing_index.is_some();

    // Only show the form when editing or when user clicks "Add"
    if !is_editing && !cfg.show_add_form {
        if ui.small_button("➕ Add new preset").clicked() {
            cfg.show_add_form = true;
        }
        return;
    }

    let title = if is_editing { "Edit Preset" } else { "New Preset" };
    ui.label(title);

    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.add(egui::TextEdit::singleline(&mut cfg.edit_name).desired_width(100.0));
        ui.label("Host:");
        ui.add(egui::TextEdit::singleline(&mut cfg.edit_host).desired_width(120.0));
    });

    let can_save = !cfg.edit_name.trim().is_empty() && !cfg.edit_host.trim().is_empty();

    ui.horizontal(|ui| {
        if is_editing {
            if ui.add_enabled(can_save, egui::Button::new("💾 Update")).clicked() {
                if let Some(idx) = cfg.editing_index {
                    sidebar.presets[idx] = TargetPreset {
                        name: cfg.edit_name.trim().to_string(),
                        host: cfg.edit_host.trim().to_string(),
                    };
                }
                clear_form(cfg);
            }
        } else {
            if ui.add_enabled(can_save, egui::Button::new("➕ Add")).clicked() {
                sidebar.presets.push(TargetPreset {
                    name: cfg.edit_name.trim().to_string(),
                    host: cfg.edit_host.trim().to_string(),
                });
                clear_form(cfg);
            }
        }
        if ui.button("Cancel").clicked() {
            clear_form(cfg);
        }
    });
}

fn clear_form(cfg: &mut ConfigState) {
    cfg.edit_name.clear();
    cfg.edit_host.clear();
    cfg.editing_index = None;
    cfg.show_add_form = false;
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
    shared.gateway_enabled = cfg.gateway_enabled;
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
