//! # Sidebar - Controls panel
//!
//! Target selector, Start/Stop, gateway detection, quick stats.
//! Export/import and notifications are delegated to their own components.

use eframe::egui;

use crate::core::config::TargetPreset;
use crate::core::pinger;
use crate::core::state::{SharedState, lock_state};

use super::export_import::ExportState;
use super::notifications::NotificationState;

/// Sidebar-specific state
pub struct SidebarState {
    pub presets: Vec<TargetPreset>,
    pub selected_preset: usize,
    pub exports: ExportState,
    pub notifications: NotificationState,
}

impl SidebarState {
    pub fn new(presets: Vec<TargetPreset>, selected_preset: usize, export_path: String) -> Self {
        Self {
            presets,
            selected_preset,
            exports: ExportState::new(export_path),
            notifications: NotificationState::new(),
        }
    }

    /// Get the currently selected target host
    pub fn selected_host(&self) -> String {
        self.presets
            .get(self.selected_preset)
            .map(|preset| preset.host.clone())
            .unwrap_or_else(|| "8.8.8.8".to_string())
    }
}

/// Snapshot of all shared state values needed by the sidebar (read in one lock)
struct SidebarSnapshot {
    running: bool,
    elapsed_display: String,
    elapsed_secs: f64,
    duration_secs: u64,
    gateway_ip: Option<String>,
    total_sent: u64,
    total_received: u64,
    loss_pct: f64,
    loss_batches: u64,
}

fn read_sidebar_snapshot(state: &SharedState) -> SidebarSnapshot {
    let shared = lock_state(&state);
    SidebarSnapshot {
        running: shared.running,
        elapsed_display: shared.elapsed_display(),
        elapsed_secs: shared.elapsed_secs(),
        duration_secs: shared.config.duration_secs,
        gateway_ip: shared.gateway.ip.clone(),
        total_sent: shared.total_sent,
        total_received: shared.total_received,
        loss_pct: shared.packet_loss_pct(),
        loss_batches: shared.loss_tracker.count,
    }
}

/// Render the full sidebar panel
pub fn render(ctx: &egui::Context, state: &SharedState, sidebar: &mut SidebarState) {
    let snapshot = read_sidebar_snapshot(state);

    super::export_import::check_auto_export_pending(state, &mut sidebar.exports);
    super::notifications::sync_and_fire(ctx, state, &sidebar.notifications);

    egui::SidePanel::left("control_panel")
        .resizable(true)
        .default_width(180.0)
        .show(ctx, |ui| {
            render_target_selector(ui, state, sidebar);
            ui.separator();
            render_start_stop(ui, state, sidebar, &snapshot);
            ui.separator();
            render_gateway(ui, state, &snapshot);
            ui.separator();
            render_quick_stats(ui, &snapshot);
            ui.separator();
            super::notifications::render_mute_toggle(ui, &mut sidebar.notifications);
            ui.separator();
            super::export_import::render(ui, state, &mut sidebar.exports);
        });
}

fn render_target_selector(ui: &mut egui::Ui, state: &SharedState, sidebar: &mut SidebarState) {
    ui.heading("🎯 Target");

    if sidebar.presets.is_empty() {
        ui.label("No presets configured");
        return;
    }

    let current_name = sidebar.presets
        .get(sidebar.selected_preset)
        .map(|preset| format!("{} ({})", preset.name, preset.host))
        .unwrap_or_else(|| "Select target".into());

    let old_selection = sidebar.selected_preset;
    egui::ComboBox::from_id_salt("target_selector")
        .selected_text(current_name)
        .width(ui.available_width() - 8.0)
        .show_ui(ui, |ui| {
            for (idx, preset) in sidebar.presets.iter().enumerate() {
                ui.selectable_value(
                    &mut sidebar.selected_preset,
                    idx,
                    format!("{} ({})", preset.name, preset.host),
                );
            }
        });

    if sidebar.selected_preset != old_selection {
        let new_host = sidebar.selected_host();
        let mut shared = lock_state(&state);
        shared.config.target = new_host;
    }
}

fn render_start_stop(ui: &mut egui::Ui, state: &SharedState, sidebar: &mut SidebarState, snap: &SidebarSnapshot) {
    let button_size = egui::vec2(ui.available_width(), 32.0);

    if snap.running {
        let btn = egui::Button::new(
            egui::RichText::new("⏹ Stop").size(16.0).strong()
        ).min_size(button_size);
        if ui.add(btn).clicked() {
            let mut shared = lock_state(&state);
            shared.flush_partial_report();
            shared.running = false;
            drop(shared);
            super::export_import::run_auto_export(state, &mut sidebar.exports);
        }
        ui.colored_label(egui::Color32::from_rgb(100, 255, 100), "● RUNNING");
        ui.label(format!("⏱ Elapsed: {}", snap.elapsed_display));

        if snap.duration_secs > 0 {
            let progress = (snap.elapsed_secs / snap.duration_secs as f64).min(1.0);
            let remaining_secs = (snap.duration_secs as f64 - snap.elapsed_secs).max(0.0) as u64;
            ui.add(egui::ProgressBar::new(progress as f32)
                .text(format!("{}m {}s remaining", remaining_secs / 60, remaining_secs % 60)));
        }
    } else {
        let btn = egui::Button::new(
            egui::RichText::new("▶ Start").size(16.0).strong()
        ).min_size(button_size);
        if ui.add(btn).clicked() {
            let mut shared = lock_state(&state);
            shared.config.target = sidebar.selected_host();
            shared.reset_data();
            shared.running = true;
        }
        ui.colored_label(egui::Color32::from_rgb(255, 100, 100), "● STOPPED");
    }
}

fn render_gateway(ui: &mut egui::Ui, state: &SharedState, snap: &SidebarSnapshot) {
    ui.heading("🌐 Gateway");

    if let Some(ip) = &snap.gateway_ip {
        ui.label(format!("Detected: {}", ip));
    } else {
        ui.label("Not detected");
    }

    if ui.button("🔍 Detect").clicked() {
        if let Some(ip) = pinger::detect_gateway() {
            let mut shared = lock_state(&state);
            shared.gateway.ip = Some(ip);
        }
    }
}

fn render_quick_stats(ui: &mut egui::Ui, snap: &SidebarSnapshot) {
    let lost = snap.total_sent - snap.total_received;
    ui.label(format!("Sent: {}", snap.total_sent));
    ui.label(format!("Received: {}", snap.total_received));
    ui.label(format!("Loss: {:.1}%", snap.loss_pct));
    ui.label(format!("Lost: {}", lost));
    ui.label(format!("Loss events: {}", snap.loss_batches));
}
