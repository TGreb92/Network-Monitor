//! # Network Monitor — GUI Layer
//!
//! Main application struct and sidebar. The Monitor tab rendering is in `monitor.rs`,
//! export logic in `export.rs`, and shared widgets in `ui_helpers.rs`.

use eframe::egui;
use std::time::Instant;

use crate::export;
use crate::monitor;
use crate::pinger;
use crate::state::{PingConfig, SharedState, PRESETS};

/// Which tab is currently active in the main panel
#[derive(PartialEq)]
enum Tab {
    Monitor,
    Console,
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
    selected_preset: usize,
}

impl NetworkMonitorApp {
    pub fn new(state: SharedState, _cc: &eframe::CreationContext<'_>) -> Self {
        let (target, timeout, interval, freq) = {
            let shared = state.lock().unwrap_or_else(|err| err.into_inner());
            (
                shared.config.target.clone(),
                shared.config.timeout_ms,
                shared.config.interval_secs,
                shared.config.ping_interval_ms,
            )
        };

        Self {
            state,
            active_tab: Tab::Monitor,
            auto_scroll: true,
            config_target: target,
            config_timeout: timeout,
            config_interval: interval,
            config_ping_freq: freq,
            selected_preset: 0,
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
        egui::SidePanel::left("config_panel")
            .resizable(true)
            .default_width(210.0)
            .show(ctx, |ui| {
                self.render_config_section(ui);
                ui.separator();
                self.render_gateway_section(ui);
                ui.separator();
                self.render_start_stop(ui);
                ui.separator();
                self.render_quick_stats(ui);
                ui.separator();
                self.render_export_section(ui);
            });
    }

    fn render_config_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("⚙ Configuration");
        ui.separator();

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
        ui.add_space(4.0);

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
        ui.add_space(8.0);

        if ui.button("✅ Apply Config").clicked() {
            self.apply_config();
        }
    }

    fn render_gateway_section(&mut self, ui: &mut egui::Ui) {
        ui.heading("🌐 Gateway");
        let (gateway_ip, gateway_enabled) = {
            let shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
            (shared.gateway_ip.clone(), shared.gateway_enabled)
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
            let mut enabled = gateway_enabled;
            if ui.checkbox(&mut enabled, "Monitor").changed() {
                let mut shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
                shared.gateway_enabled = enabled;
            }
        });
    }

    fn render_start_stop(&mut self, ui: &mut egui::Ui) {
        let running = {
            let shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
            shared.running
        };

        if running {
            if ui.button("⏹ Stop").clicked() {
                self.state.lock().unwrap_or_else(|err| err.into_inner()).running = false;
            }
            ui.colored_label(egui::Color32::from_rgb(100, 255, 100), "● RUNNING");
        } else {
            if ui.button("▶ Start").clicked() {
                self.state.lock().unwrap_or_else(|err| err.into_inner()).running = true;
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
            });
            ui.separator();

            match self.active_tab {
                Tab::Monitor => {
                    let shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
                    monitor::render(ui, &shared);
                }
                Tab::Console => self.render_console(ui),
            }
        });
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

    fn apply_config(&mut self) {
        let mut shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
        shared.config = PingConfig {
            target: self.config_target.clone(),
            timeout_ms: self.config_timeout,
            interval_secs: self.config_interval,
            ping_interval_ms: self.config_ping_freq.max(100),
        };
        shared.config_changed = true;
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
