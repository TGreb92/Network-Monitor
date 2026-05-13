//! # Console Tab - Live ping log
//!
//! Scrolling log of individual ping results with color coding
//! and auto-scroll toggle.

use eframe::egui;

use crate::core::state::{SharedState, lock_state};

/// Console-specific state
pub struct ConsoleState {
    pub auto_scroll: bool,
}

impl ConsoleState {
    pub fn new() -> Self {
        Self { auto_scroll: true }
    }
}

/// Render the Console tab contents
pub fn render(ui: &mut egui::Ui, state: &SharedState, console: &mut ConsoleState) {
    render_toolbar(ui, state, console);
    ui.separator();
    render_log(ui, state, console);
}

fn render_toolbar(ui: &mut egui::Ui, state: &SharedState, console: &mut ConsoleState) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut console.auto_scroll, "Auto-scroll");
        let running = {
            let shared = lock_state(&state);
            shared.running
        };
        if running {
            ui.colored_label(egui::Color32::from_rgb(100, 255, 100), "🟢 LIVE");
        } else {
            ui.colored_label(egui::Color32::from_rgb(255, 100, 100), "🔴 STOPPED");
        }
    });
}

fn render_log(ui: &mut egui::Ui, state: &SharedState, console: &ConsoleState) {
    use crate::core::state::PingTier;

    let (entries, thresholds) = {
        let shared = lock_state(&state);
        let entries: Vec<(String, Option<f64>, bool)> = shared.log_entries
            .iter()
            .map(|e| (e.message.clone(), e.latency_ms, e.success))
            .collect();
        let thresholds = shared.thresholds.clone();
        (entries, thresholds)
    };

    let scroll = egui::ScrollArea::vertical().auto_shrink([false; 2]);
    let scroll = if console.auto_scroll { scroll.stick_to_bottom(true) } else { scroll };

    scroll.show(ui, |ui| {
        ui.style_mut().override_font_id = Some(egui::FontId::monospace(12.0));
        for (msg, latency_ms, success) in &entries {
            let color = if !success {
                egui::Color32::from_rgb(255, 100, 100)
            } else {
                match PingTier::classify(*latency_ms, &thresholds) {
                    PingTier::Critical => egui::Color32::from_rgb(255, 80, 80),
                    PingTier::High => egui::Color32::from_rgb(255, 150, 50),
                    PingTier::Elevated => egui::Color32::from_rgb(200, 200, 50),
                    PingTier::Normal => egui::Color32::from_rgb(180, 220, 180),
                }
            };
            ui.colored_label(color, msg);
        }
    });
}
