//! # Config Tab - Settings with TOML persistence
//!
//! Ping parameters, gateway settings, and duration.
//! Preset management is in `presets.rs`.

use eframe::egui;
use std::time::Instant;

use crate::core::config::{self, SavedConfig, default_presets};
use crate::core::state::{PingConfig, SharedState, lock_state};
use crate::ui::components::presets::{self, PresetEditorState};
use crate::ui::components::sidebar::SidebarState;

/// Config tab local state
pub struct ConfigState {
    pub timeout: u32,
    pub interval: u64,
    pub ping_freq: u64,
    pub gateway_enabled: bool,
    pub auto_detect_gateway: bool,
    pub duration_mins: u64,
    pub export_path: String,
    pub auto_export_csv: bool,
    pub auto_export_json: bool,
    pub auto_export_isp: bool,
    pub auto_export_log: bool,
    pub notify_on_loss: bool,
    pub notify_on_gw_loss: bool,
    pub notify_on_elevated_ping: bool,
    pub notify_on_high_ping: bool,
    pub notify_on_critical_ping: bool,
    pub threshold_elevated: u32,
    pub threshold_high: u32,
    pub threshold_critical: u32,
    pub modem_health_enabled: bool,
    pub modem_health_url: String,
    pub modem_health_interval: u32,
    pub modem_struggle_window: u32,
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
            export_path: saved.export_path.clone(),
            auto_export_csv: saved.auto_export_csv,
            auto_export_json: saved.auto_export_json,
            auto_export_isp: saved.auto_export_isp,
            auto_export_log: saved.auto_export_log,
            notify_on_loss: saved.notify_on_loss,
            notify_on_gw_loss: saved.notify_on_gw_loss,
            notify_on_elevated_ping: saved.notify_on_elevated_ping,
            notify_on_high_ping: saved.notify_on_high_ping,
            notify_on_critical_ping: saved.notify_on_critical_ping,
            threshold_elevated: saved.threshold_elevated_ms,
            threshold_high: saved.threshold_high_ms,
            threshold_critical: saved.threshold_critical_ms,
            modem_health_enabled: saved.modem_health_enabled,
            modem_health_url: saved.modem_health_url.clone(),
            modem_health_interval: saved.modem_health_interval_secs,
            modem_struggle_window: saved.modem_struggle_window_mins,
            status: None,
            preset_editor: PresetEditorState::new(),
        }
    }
}

/// Render the Config tab contents
pub fn render(ui: &mut egui::Ui, state: &SharedState, cfg: &mut ConfigState, sidebar: &mut SidebarState) {
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
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
        });

    // Sync export settings to sidebar so exports use the latest values
    sidebar.exports.export_path = cfg.export_path.clone();
    sidebar.exports.auto_export_csv = cfg.auto_export_csv;
    sidebar.exports.auto_export_json = cfg.auto_export_json;
    sidebar.exports.auto_export_isp = cfg.auto_export_isp;
    sidebar.exports.auto_export_log = cfg.auto_export_log;
    sidebar.notifications.notify_on_loss = cfg.notify_on_loss;
    sidebar.notifications.notify_on_gw_loss = cfg.notify_on_gw_loss;
    sidebar.notifications.notify_on_elevated_ping = cfg.notify_on_elevated_ping;
    sidebar.notifications.notify_on_high_ping = cfg.notify_on_high_ping;
    sidebar.notifications.notify_on_critical_ping = cfg.notify_on_critical_ping;
    sidebar.notifications.threshold_elevated_ms = cfg.threshold_elevated;
    sidebar.notifications.threshold_high_ms = cfg.threshold_high;
    sidebar.notifications.threshold_critical_ms = cfg.threshold_critical;

    // Sync modem health config to shared state
    {
        let mut shared = lock_state(state);
        shared.modem_health_enabled = cfg.modem_health_enabled;
        shared.modem_health_url = cfg.modem_health_url.clone();
        shared.modem_health_interval_secs = cfg.modem_health_interval;
        shared.modem_struggle_window_mins = cfg.modem_struggle_window;
        if !cfg.modem_health_enabled {
            shared.modem_http_status = crate::core::state::ModemHttpStatus::Disabled;
        }
    }
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
    ui.add_space(8.0);

    ui.heading("📁 Export");
    ui.add_space(4.0);
    ui.label("Export folder (empty = exe_dir/exports/):");
    ui.horizontal(|ui| {
        let display = if cfg.export_path.is_empty() {
            "(default)".to_string()
        } else {
            cfg.export_path.clone()
        };
        ui.monospace(&display);
        if ui.button("📂 Browse…").clicked() {
            if let Some(folder) = rfd::FileDialog::new()
                .set_title("Select export folder")
                .pick_folder()
            {
                cfg.export_path = folder.display().to_string();
            }
        }
        if !cfg.export_path.is_empty() && ui.button("🔄 Reset").on_hover_text("Reset to default (exe_dir/exports/)").clicked() {
            cfg.export_path.clear();
        }
    });

    ui.add_space(8.0);
    ui.label("Auto-export on stop:");
    ui.horizontal(|ui| {
        ui.checkbox(&mut cfg.auto_export_csv, "CSV");
        ui.checkbox(&mut cfg.auto_export_json, "JSON");
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut cfg.auto_export_isp, "ISP Report");
        ui.checkbox(&mut cfg.auto_export_log, "Console Log");
    });

    ui.add_space(8.0);
    ui.heading("🔔 Notifications");
    ui.add_space(4.0);
    ui.checkbox(&mut cfg.notify_on_loss, "Toast on loss event");
    ui.checkbox(&mut cfg.notify_on_gw_loss, "Toast on gateway loss");

    ui.add_space(4.0);
    ui.label("Latency tiers:");

    ui.horizontal(|ui| {
        ui.checkbox(&mut cfg.notify_on_elevated_ping, "Elevated");
        ui.add(egui::DragValue::new(&mut cfg.threshold_elevated)
            .range(1..=cfg.timeout)
            .suffix(" ms").speed(5));
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut cfg.notify_on_high_ping, "High");
        ui.add(egui::DragValue::new(&mut cfg.threshold_high)
            .range(1..=cfg.timeout)
            .suffix(" ms").speed(5));
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut cfg.notify_on_critical_ping, "Critical");
        ui.add(egui::DragValue::new(&mut cfg.threshold_critical)
            .range(1..=cfg.timeout)
            .suffix(" ms").speed(5));
    });

    // Auto-fix ordering: elevated <= high <= critical
    if cfg.threshold_high < cfg.threshold_elevated {
        cfg.threshold_high = cfg.threshold_elevated;
    }
    if cfg.threshold_critical < cfg.threshold_high {
        cfg.threshold_critical = cfg.threshold_high;
    }

    ui.add_space(8.0);
    ui.heading("🔌 Modem Health Check");
    ui.add_space(4.0);
    ui.checkbox(&mut cfg.modem_health_enabled, "Enable HTTP health check");
    ui.label("URL (HTTP only):");
    ui.text_edit_singleline(&mut cfg.modem_health_url);
    ui.horizontal(|ui| {
        ui.label("Check interval:");
        ui.add(egui::DragValue::new(&mut cfg.modem_health_interval)
            .range(5..=120)
            .suffix(" s").speed(1));
    });
    ui.horizontal(|ui| {
        ui.label("Struggle detection window:");
        ui.add(egui::DragValue::new(&mut cfg.modem_struggle_window)
            .range(2..=30)
            .suffix(" min").speed(1));
    });
    ui.label(egui::RichText::new("Triggers when 3+ loss events occur within this window while gateway is healthy.").weak().small());
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

fn apply_and_save(state: &SharedState, cfg: &mut ConfigState, sidebar: &mut SidebarState) {
    let target = sidebar.selected_host();
    let preset = sidebar.presets.get(sidebar.selected_preset);
    let use_tcp = preset
        .map(|p| p.mode == crate::core::config::TestMode::Tcp)
        .unwrap_or(false);
    let tcp_port = preset
        .map(|p| p.port)
        .unwrap_or(443);
    let mut shared = lock_state(&state);
    shared.config = PingConfig {
        target,
        timeout_ms: cfg.timeout,
        interval_secs: cfg.interval,
        ping_interval_ms: cfg.ping_freq.max(100),
        duration_secs: cfg.duration_mins * 60,
        use_tcp,
        tcp_port,
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
        export_path: cfg.export_path.clone(),
        auto_export_csv: cfg.auto_export_csv,
        auto_export_json: cfg.auto_export_json,
        auto_export_isp: cfg.auto_export_isp,
        auto_export_log: cfg.auto_export_log,
        notify_on_loss: cfg.notify_on_loss,
        notify_on_gw_loss: cfg.notify_on_gw_loss,
        notify_on_elevated_ping: cfg.notify_on_elevated_ping,
        notify_on_high_ping: cfg.notify_on_high_ping,
        notify_on_critical_ping: cfg.notify_on_critical_ping,
        threshold_elevated_ms: cfg.threshold_elevated,
        threshold_high_ms: cfg.threshold_high,
        threshold_critical_ms: cfg.threshold_critical,
        modem_health_enabled: cfg.modem_health_enabled,
        modem_health_url: cfg.modem_health_url.clone(),
        modem_health_interval_secs: cfg.modem_health_interval,
        modem_struggle_window_mins: cfg.modem_struggle_window,
    };
    match config::save(&saved) {
        Ok(()) => cfg.status = Some(("✅ Config saved".into(), Instant::now())),
        Err(err) => cfg.status = Some((format!("❌ Save failed: {}", err), Instant::now())),
    }

    // Push notification config into the live notification state
    sidebar.notifications.notify_on_loss = cfg.notify_on_loss;
    sidebar.notifications.notify_on_gw_loss = cfg.notify_on_gw_loss;
    sidebar.notifications.notify_on_elevated_ping = cfg.notify_on_elevated_ping;
    sidebar.notifications.notify_on_high_ping = cfg.notify_on_high_ping;
    sidebar.notifications.notify_on_critical_ping = cfg.notify_on_critical_ping;
    sidebar.notifications.threshold_elevated_ms = cfg.threshold_elevated;
    sidebar.notifications.threshold_high_ms = cfg.threshold_high;
    sidebar.notifications.threshold_critical_ms = cfg.threshold_critical;
}
