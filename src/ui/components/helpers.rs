//! # UI Helpers - Shared widgets and color functions
//!
//! Reusable UI primitives used across the app: stat cards and
//! traffic-light color mappings for loss, latency, and jitter.

use eframe::egui;

/// Render a compact stat card with a label and a large colored value
pub fn stat_card(ui: &mut egui::Ui, label: &str, value: &str, color: egui::Color32) {
    egui::Frame::new()
        .inner_margin(egui::Margin::same(8))
        .corner_radius(4.0)
        .fill(egui::Color32::from_rgb(40, 40, 50))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.small(label);
                ui.colored_label(color, egui::RichText::new(value).size(18.0).strong());
            });
        });
}

/// Map packet loss percentage to a traffic-light color
pub fn loss_color(loss: f64) -> egui::Color32 {
    if loss < 1.0 {
        egui::Color32::from_rgb(100, 255, 100)
    } else if loss < 5.0 {
        egui::Color32::from_rgb(255, 255, 100)
    } else {
        egui::Color32::from_rgb(255, 80, 80)
    }
}

/// Map latency to a traffic-light color
pub fn latency_color(ms: f64) -> egui::Color32 {
    if ms < 30.0 {
        egui::Color32::from_rgb(100, 255, 100)
    } else if ms < 100.0 {
        egui::Color32::from_rgb(255, 255, 100)
    } else {
        egui::Color32::from_rgb(255, 80, 80)
    }
}

/// Map jitter to a traffic-light color: green (<5ms), yellow (5-20ms), red (>20ms)
pub fn jitter_color(ms: f64) -> egui::Color32 {
    if ms < 5.0 {
        egui::Color32::from_rgb(100, 255, 100)
    } else if ms < 20.0 {
        egui::Color32::from_rgb(255, 255, 100)
    } else {
        egui::Color32::from_rgb(255, 80, 80)
    }
}
