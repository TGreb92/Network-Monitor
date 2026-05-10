//! # Network Monitor — GUI Layer
//!
//! Main application struct with three tabs: Monitor, Console, Config.
//! Monitor rendering is in `monitor.rs`, export in `export.rs`,
//! shared widgets in `ui_helpers.rs`, config persistence in `config.rs`.

use eframe::egui;
use std::time::Instant;

use crate::config::{self, SavedConfig};
use crate::export;
use crate::help;
use crate::monitor;
use crate::pinger;
use crate::state::{PingConfig, SharedState, PRESETS};

/// Which tab is currently active in the main panel
#[derive(PartialEq)]
enum Tab {
    Monitor,
    Console,
    Config,
    Help,
}

/// Main application struct holding shared state and UI-local state
pub struct NetworkMonitorApp {
    state: SharedState,
    active_tab: Tab,
    auto_scroll: bool,
    config_target: String,
    config_timeout: u32,
    config_interval: u64,
    config_ping_freq: u64,
    config_gateway_enabled: bool,
    config_auto_detect_gateway: bool,
    config_duration_mins: u64,
    selected_preset: usize,
    config_status: Option<(String, Instant)>,
}

impl NetworkMonitorApp {
    pub fn new(state: SharedState, _cc: &eframe::CreationContext<'_>) -> Self {
        let saved = config::load();

        // Apply loaded config to shared state
        {
            let mut shared = state.lock().unwrap_or_else(|err| err.into_inner());
            shared.config = PingConfig::from_saved(&saved);
            shared.gateway_enabled = saved.gateway_enabled;

            // Auto-detect gateway on startup if enabled
            if saved.auto_detect_gateway {
                if let Some(ip) = pinger::detect_gateway() {
                    shared.gateway_ip = Some(ip);
                }
            }
        }

        Self {
            state,
            active_tab: Tab::Monitor,
            auto_scroll: true,
            config_target: saved.target,
            config_timeout: saved.timeout_ms,
            config_interval: saved.interval_secs,
            config_ping_freq: saved.ping_interval_ms,
            config_gateway_enabled: saved.gateway_enabled,
            config_auto_detect_gateway: saved.auto_detect_gateway,
            config_duration_mins: saved.duration_mins,
            selected_preset: 0,
            config_status: None,
        }
    }
}

impl eframe::App for NetworkMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
        self.render_sidebar(ctx);
        self.render_main(ctx);
    }
}

impl NetworkMonitorApp {
    fn render_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("control_panel")
            .resizable(true)
            .default_width(180.0)
            .show(ctx, |ui| {
                self.render_start_stop(ui);
                ui.separator();
                self.render_gateway_section(ui);
                ui.separator();
                self.render_quick_stats(ui);
                ui.separator();
                self.render_export_section(ui);
            });
    }

    fn render_gateway_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("🌐 Gateway");
        let gateway_ip = {
            let shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
            shared.gateway_ip.clone()
        };

        match &gateway_ip {
            Some(ip) => { ui.label(format!("Detected: {}", ip)); }
            None => { ui.label("Not detected"); }
        }

        ui.horizontal(|ui| {
            if ui.button("🔍 Detect").clicked() {
                if let Some(ip) = pinger::detect_gateway() {
                    let mut shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
                    shared.gateway_ip = Some(ip);
                }
            }
        });
    }

    fn render_start_stop(&mut self, ui: &mut egui::Ui) {
        let (running, elapsed_display, duration_secs, elapsed_secs) = {
            let shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
            (shared.running, shared.elapsed_display(), shared.config.duration_secs, shared.elapsed_secs())
        };

        let button_size = egui::vec2(ui.available_width(), 32.0);

        if running {
            let btn = egui::Button::new(
                egui::RichText::new("⏹ Stop").size(16.0).strong()
            ).min_size(button_size);
            if ui.add(btn).clicked() {
                let mut shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
                shared.flush_partial_report();
                shared.running = false;
            }
            ui.colored_label(egui::Color32::from_rgb(100, 255, 100), "● RUNNING");

            // Show elapsed time
            ui.label(format!("⏱ Elapsed: {}", elapsed_display));

            // Show progress if duration is set
            if duration_secs > 0 {
                let progress = (elapsed_secs / duration_secs as f64).min(1.0);
                let remaining_secs = (duration_secs as f64 - elapsed_secs).max(0.0) as u64;
                let remaining_mins = remaining_secs / 60;
                let remaining_sec = remaining_secs % 60;
                ui.add(egui::ProgressBar::new(progress as f32)
                    .text(format!("{}m {}s remaining", remaining_mins, remaining_sec)));
            }
        } else {
            let btn = egui::Button::new(
                egui::RichText::new("▶ Start").size(16.0).strong()
            ).min_size(button_size);
            if ui.add(btn).clicked() {
                let mut shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
                shared.reset_data();
                shared.running = true;
            }
            ui.colored_label(egui::Color32::from_rgb(255, 100, 100), "● STOPPED");
        }
    }

    fn render_quick_stats(&self, ui: &mut egui::Ui) {
        let (sent, recv, loss) = {
            let shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
            (shared.total_sent, shared.total_received, shared.packet_loss_pct())
        };
        ui.label(format!("Sent: {}", sent));
        ui.label(format!("Received: {}", recv));
        ui.label(format!("Loss: {:.1}%", loss));
    }

    fn render_export_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("📥 Export");
        ui.horizontal(|ui| {
            if ui.button("CSV").clicked() { self.do_export_csv(); }
            if ui.button("JSON").clicked() { self.do_export_json(); }
        });

        let message = {
            let shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
            shared.export_message.clone()
        };
        if let Some((text, when)) = message {
            if when.elapsed().as_secs() < 5 {
                ui.colored_label(egui::Color32::from_rgb(150, 200, 150), &text);
            }
        }
    }

    fn render_main(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Monitor, "📊 Monitor");
                ui.selectable_value(&mut self.active_tab, Tab::Console, "🖥 Console");
                ui.selectable_value(&mut self.active_tab, Tab::Config, "⚙ Config");
                ui.selectable_value(&mut self.active_tab, Tab::Help, "❓ Help");
            });
            ui.separator();

            match self.active_tab {
                Tab::Monitor => {
                    let shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
                    monitor::render(ui, &shared);
                }
                Tab::Console => self.render_console(ui),
                Tab::Config => self.render_config_tab(ui),
                Tab::Help => help::render(ui),
            }
        });
    }

    fn render_config_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading("⚙ Configuration");
        ui.add_space(8.0);

        // Target presets
        ui.label("Quick presets:");
        egui::ComboBox::from_id_salt("preset_combo")
            .selected_text(format!(
                "{} ({})",
                PRESETS[self.selected_preset].0,
                PRESETS[self.selected_preset].1
            ))
            .show_ui(ui, |ui| {
                for (idx, (ip, name)) in PRESETS.iter().enumerate() {
                    if ui.selectable_value(&mut self.selected_preset, idx, format!("{ip} ({name})")).clicked() {
                        self.config_target = ip.to_string();
                    }
                }
            });
        ui.add_space(8.0);

        // Editable fields
        ui.label("Target host:");
        ui.text_edit_singleline(&mut self.config_target);
        ui.add_space(4.0);

        ui.label("Timeout (ms):");
        ui.add(egui::DragValue::new(&mut self.config_timeout).range(100..=30000).speed(50).suffix(" ms"));
        ui.add_space(4.0);

        ui.label("Report interval (s):");
        ui.add(egui::DragValue::new(&mut self.config_interval).range(5..=3600).speed(1).suffix(" s"));
        ui.add_space(4.0);

        ui.label("Ping frequency (ms):");
        ui.add(egui::DragValue::new(&mut self.config_ping_freq).range(100..=10000).speed(50).suffix(" ms"));
        ui.add_space(4.0);

        ui.checkbox(&mut self.config_gateway_enabled, "Enable gateway monitoring");
        ui.checkbox(&mut self.config_auto_detect_gateway, "Auto-detect gateway on startup");
        ui.add_space(8.0);

        ui.label("Test duration (minutes, 0 = unlimited):");
        ui.add(egui::DragValue::new(&mut self.config_duration_mins).range(0..=1440).speed(1).suffix(" min"));
        ui.add_space(12.0);

        // Apply & Save buttons
        ui.horizontal(|ui| {
            if ui.button("✅ Apply & Save").clicked() {
                self.apply_and_save_config();
            }
            if ui.button("🔄 Reset to Defaults").clicked() {
                self.reset_config_to_defaults();
            }
        });

        // Status message
        if let Some((text, when)) = &self.config_status {
            if when.elapsed().as_secs() < 5 {
                ui.add_space(8.0);
                ui.colored_label(egui::Color32::from_rgb(150, 200, 150), text);
            }
        }
    }

    fn render_console(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.auto_scroll, "Auto-scroll");
            let running = {
                let shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
                shared.running
            };
            if running {
                ui.colored_label(egui::Color32::from_rgb(100, 255, 100), "🟢 LIVE");
            } else {
                ui.colored_label(egui::Color32::from_rgb(255, 100, 100), "🔴 STOPPED");
            }
        });
        ui.separator();

        let log_messages: Vec<String> = {
            let shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
            shared.log_entries.iter().map(|entry| entry.message.clone()).collect()
        };

        let scroll = egui::ScrollArea::vertical().auto_shrink([false; 2]);
        let scroll = if self.auto_scroll { scroll.stick_to_bottom(true) } else { scroll };

        scroll.show(ui, |ui| {
            ui.style_mut().override_font_id = Some(egui::FontId::monospace(12.0));
            for msg in &log_messages {
                let color = if msg.contains("timed out") || msg.contains("unreachable") {
                    egui::Color32::from_rgb(255, 100, 100)
                } else {
                    egui::Color32::from_rgb(180, 220, 180)
                };
                ui.colored_label(color, msg);
            }
        });
    }

    fn apply_and_save_config(&mut self) {
        // Apply to shared state
        let mut shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
        shared.config = PingConfig {
            target: self.config_target.clone(),
            timeout_ms: self.config_timeout,
            interval_secs: self.config_interval,
            ping_interval_ms: self.config_ping_freq.max(100),
            duration_secs: self.config_duration_mins * 60,
        };
        shared.gateway_enabled = self.config_gateway_enabled;
        shared.config_changed = true;
        drop(shared);

        // Save to disk
        let saved = SavedConfig {
            target: self.config_target.clone(),
            timeout_ms: self.config_timeout,
            interval_secs: self.config_interval,
            ping_interval_ms: self.config_ping_freq.max(100),
            gateway_enabled: self.config_gateway_enabled,
            auto_detect_gateway: self.config_auto_detect_gateway,
            duration_mins: self.config_duration_mins,
        };
        match config::save(&saved) {
            Ok(()) => self.config_status = Some(("✅ Config saved".into(), Instant::now())),
            Err(err) => self.config_status = Some((format!("❌ Save failed: {}", err), Instant::now())),
        }
    }

    fn reset_config_to_defaults(&mut self) {
        let defaults = SavedConfig::default();
        self.config_target = defaults.target;
        self.config_timeout = defaults.timeout_ms;
        self.config_interval = defaults.interval_secs;
        self.config_ping_freq = defaults.ping_interval_ms;
        self.config_gateway_enabled = defaults.gateway_enabled;
        self.config_auto_detect_gateway = defaults.auto_detect_gateway;
        self.config_duration_mins = defaults.duration_mins;
    }

    fn do_export_csv(&mut self) {
        let shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let filename = format!("network-monitor-{}.csv", timestamp);
        let path = export::export_dir().join(&filename);
        let result = export::write_csv(&path, &shared);
        drop(shared);
        self.set_export_message(&filename, result);
    }

    fn do_export_json(&mut self) {
        let shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let filename = format!("network-monitor-{}.json", timestamp);
        let path = export::export_dir().join(&filename);
        let result = export::write_json(&path, &shared);
        drop(shared);
        self.set_export_message(&filename, result);
    }

    fn set_export_message(&self, filename: &str, result: std::io::Result<()>) {
        let mut shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
        match result {
            Ok(()) => shared.export_message = Some((format!("✅ Saved {}", filename), Instant::now())),
            Err(err) => shared.export_message = Some((format!("❌ {}", err), Instant::now())),
        }
    }
}
