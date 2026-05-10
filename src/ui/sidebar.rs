//! # Sidebar — Controls panel
//!
//! Start/Stop, gateway detection, quick stats, and export buttons.

use eframe::egui;
use std::time::Instant;

use crate::core::export;
use crate::core::pinger;
use crate::core::state::SharedState;

/// Sidebar-specific state
pub struct SidebarState {
    pub export_status: Option<(String, Instant)>,
}

impl SidebarState {
    pub fn new() -> Self {
        Self { export_status: None }
    }
}

/// Render the full sidebar panel
pub fn render(ctx: &egui::Context, state: &SharedState, sidebar: &mut SidebarState) {
    egui::SidePanel::left("control_panel")
        .resizable(true)
        .default_width(180.0)
        .show(ctx, |ui| {
            render_start_stop(ui, state);
            ui.separator();
            render_gateway(ui, state);
            ui.separator();
            render_quick_stats(ui, state);
            ui.separator();
            render_export(ui, state, sidebar);
        });
}

fn render_start_stop(ui: &mut egui::Ui, state: &SharedState) {
    let (running, elapsed_display, duration_secs, elapsed_secs) = {
        let shared = state.lock().unwrap_or_else(|err| err.into_inner());
        (shared.running, shared.elapsed_display(), shared.config.duration_secs, shared.elapsed_secs())
    };

    let button_size = egui::vec2(ui.available_width(), 32.0);

    if running {
        let btn = egui::Button::new(
            egui::RichText::new("⏹ Stop").size(16.0).strong()
        ).min_size(button_size);
        if ui.add(btn).clicked() {
            let mut shared = state.lock().unwrap_or_else(|err| err.into_inner());
            shared.flush_partial_report();
            shared.running = false;
        }
        ui.colored_label(egui::Color32::from_rgb(100, 255, 100), "● RUNNING");
        ui.label(format!("⏱ Elapsed: {}", elapsed_display));

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
            let mut shared = state.lock().unwrap_or_else(|err| err.into_inner());
            shared.reset_data();
            shared.running = true;
        }
        ui.colored_label(egui::Color32::from_rgb(255, 100, 100), "● STOPPED");
    }
}

fn render_gateway(ui: &mut egui::Ui, state: &SharedState) {
    ui.heading("🌐 Gateway");
    let gateway_ip = {
        let shared = state.lock().unwrap_or_else(|err| err.into_inner());
        shared.gateway_ip.clone()
    };

    match &gateway_ip {
        Some(ip) => { ui.label(format!("Detected: {}", ip)); }
        None => { ui.label("Not detected"); }
    }

    if ui.button("🔍 Detect").clicked() {
        if let Some(ip) = pinger::detect_gateway() {
            let mut shared = state.lock().unwrap_or_else(|err| err.into_inner());
            shared.gateway_ip = Some(ip);
        }
    }
}

fn render_quick_stats(ui: &mut egui::Ui, state: &SharedState) {
    let (sent, recv, loss) = {
        let shared = state.lock().unwrap_or_else(|err| err.into_inner());
        (shared.total_sent, shared.total_received, shared.packet_loss_pct())
    };
    ui.label(format!("Sent: {}", sent));
    ui.label(format!("Received: {}", recv));
    ui.label(format!("Loss: {:.1}%", loss));
}

fn render_export(ui: &mut egui::Ui, state: &SharedState, sidebar: &mut SidebarState) {
    ui.heading("📥 Export");
    ui.horizontal(|ui| {
        if ui.button("CSV").clicked() {
            do_export(state, sidebar, "csv");
        }
        if ui.button("JSON").clicked() {
            do_export(state, sidebar, "json");
        }
    });

    // Show status message for 5 seconds
    if let Some((text, when)) = &sidebar.export_status {
        if when.elapsed().as_secs() < 5 {
            ui.colored_label(egui::Color32::from_rgb(150, 200, 150), text);
        }
    }
}

fn do_export(state: &SharedState, sidebar: &mut SidebarState, format: &str) {
    let shared = state.lock().unwrap_or_else(|err| err.into_inner());
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("network-monitor-{}.{}", timestamp, format);
    let path = export::export_dir().join(&filename);

    let result = match format {
        "csv" => export::write_csv(&path, &shared),
        "json" => export::write_json(&path, &shared),
        _ => Ok(()),
    };
    drop(shared);

    match result {
        Ok(()) => sidebar.export_status = Some((format!("✅ Saved {}", filename), Instant::now())),
        Err(err) => sidebar.export_status = Some((format!("❌ {}", err), Instant::now())),
    }
}
