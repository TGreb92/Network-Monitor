//! # Sidebar - Controls panel
//!
//! Target selector, Start/Stop, gateway detection, quick stats, and export.

use eframe::egui;
use std::time::Instant;

use crate::core::config::TargetPreset;
use crate::core::export;
use crate::core::pinger;
use crate::core::state::{SharedState, lock_state};

/// Sidebar-specific state
pub struct SidebarState {
    pub export_status: Option<(String, Instant)>,
    pub presets: Vec<TargetPreset>,
    pub selected_preset: usize,
    pub export_path: String,
    pub auto_export_csv: bool,
    pub auto_export_json: bool,
    pub auto_export_isp: bool,
    pub auto_export_log: bool,
    pub notify_on_loss: bool,
}

impl SidebarState {
    pub fn new(presets: Vec<TargetPreset>, selected_preset: usize, export_path: String) -> Self {
        Self {
            export_status: None,
            presets,
            selected_preset,
            export_path,
            auto_export_csv: false,
            auto_export_json: false,
            auto_export_isp: false,
            auto_export_log: false,
            notify_on_loss: false,
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

    // Check if pinger auto-stopped and needs auto-export
    check_auto_export_pending(state, sidebar);

    // Sync notification setting and check for pending loss notifications
    sync_and_check_notifications(ctx, state, sidebar);

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
            render_export(ui, state, sidebar);
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

    // Apply new target to shared state if selection changed
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
            run_auto_export_if_enabled(state, sidebar);
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

fn render_export(ui: &mut egui::Ui, state: &SharedState, sidebar: &mut SidebarState) {
    ui.heading("📥 Export");
    ui.horizontal(|ui| {
        if ui.button("CSV").clicked() { do_export(state, sidebar, "csv"); }
        if ui.button("JSON").clicked() { do_export(state, sidebar, "json"); }
    });
    if ui.button("📋 ISP Report").on_hover_text("Human-readable report for your ISP").clicked() {
        do_export(state, sidebar, "txt");
    }
    if ui.button("🖥 Console Log").on_hover_text("Raw ping log output").clicked() {
        do_export(state, sidebar, "log");
    }

    ui.add_space(8.0);
    ui.separator();
    ui.heading("📂 Import");
    if ui.button("📥 Load JSON…").on_hover_text("Import a previous JSON export to review").clicked() {
        do_import(state, sidebar);
    }

    // Show status message for 5 seconds
    if let Some((text, when)) = &sidebar.export_status {
        if when.elapsed().as_secs() < 5 {
            ui.colored_label(egui::Color32::from_rgb(150, 200, 150), text);
        }
    }
}

fn do_export(state: &SharedState, sidebar: &mut SidebarState, format: &str) {
    // Clone state and release lock before doing file I/O
    let snapshot = lock_state(state).clone();

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("network-monitor-{}.{}", timestamp, format);
    let export_dir = match export::export_dir(&sidebar.export_path) {
        Ok(dir) => dir,
        Err(err) => {
            sidebar.export_status = Some((format!("❌ {}", err), Instant::now()));
            return;
        }
    };
    let path = export_dir.join(&filename);

    let result = match format {
        "csv" => export::write_csv(&path, &snapshot),
        "json" => export::write_json(&path, &snapshot),
        "txt" => export::write_isp_report(&path, &snapshot),
        "log" => export::write_console_log(&path, &snapshot),
        _ => Ok(()),
    };

    match result {
        Ok(()) => sidebar.export_status = Some((format!("✅ Saved {}", filename), Instant::now())),
        Err(err) => sidebar.export_status = Some((format!("❌ {}", err), Instant::now())),
    }
}

fn do_import(state: &SharedState, sidebar: &mut SidebarState) {
    let file = rfd::FileDialog::new()
        .set_title("Import JSON export")
        .add_filter("JSON files", &["json"])
        .pick_file();

    let Some(path) = file else {
        return;
    };

    match export::read_json(&path) {
        Ok(imported) => {
            let mut shared = lock_state(&state);
            // Stop any running test before replacing state
            shared.running = false;
            // Replace all data with imported data
            shared.config = imported.config;
            shared.results = imported.results;
            shared.log_entries = imported.log_entries;
            shared.interval_reports = imported.interval_reports;
            shared.all_latencies = imported.all_latencies;
            shared.total_sent = imported.total_sent;
            shared.total_received = imported.total_received;
            shared.seq_counter = imported.seq_counter;
            shared.jitter = imported.jitter;
            shared.gateway = imported.gateway;
            shared.loss_tracker = imported.loss_tracker;
            shared.interval = imported.interval;
            shared.start_time = None;
            shared.config_changed = false;

            let filename = path.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".into());
            sidebar.export_status = Some((
                format!("✅ Imported {}", filename),
                Instant::now(),
            ));
        }
        Err(err) => {
            sidebar.export_status = Some((format!("❌ {}", err), Instant::now()));
        }
    }
}

/// Check if the pinger auto-stopped and there's a pending auto-export
fn check_auto_export_pending(state: &SharedState, sidebar: &mut SidebarState) {
    let pending = {
        let shared = lock_state(&state);
        shared.auto_export_pending
    };

    if pending {
        run_auto_export_if_enabled(state, sidebar);
        let mut shared = lock_state(&state);
        shared.auto_export_pending = false;
    }
}

/// Run auto-export for all enabled formats
fn run_auto_export_if_enabled(state: &SharedState, sidebar: &mut SidebarState) {
    let any_enabled = sidebar.auto_export_csv || sidebar.auto_export_json
        || sidebar.auto_export_isp || sidebar.auto_export_log;
    if !any_enabled {
        return;
    }

    // Clone state and release lock before doing file I/O
    let snapshot = lock_state(state).clone();

    let msg = export::run_auto_export(
        &snapshot,
        &sidebar.export_path,
        sidebar.auto_export_csv,
        sidebar.auto_export_json,
        sidebar.auto_export_isp,
        sidebar.auto_export_log,
    );

    sidebar.export_status = Some((msg, Instant::now()));
}

/// Sync notification config to shared state and check for pending loss notifications
fn sync_and_check_notifications(ctx: &egui::Context, state: &SharedState, sidebar: &SidebarState) {
    let mut shared = lock_state(&state);
    shared.notify_loss_enabled = sidebar.notify_on_loss;

    if shared.notify_loss_pending {
        shared.notify_loss_pending = false;
        let target = shared.config.target.clone();
        let loss_count = shared.loss_tracker.count;
        drop(shared);

        show_loss_notification(ctx, &target, loss_count);
    }
}

/// Show a Windows toast notification for a loss event
fn show_loss_notification(_ctx: &egui::Context, target: &str, loss_count: u64) {
    let body = format!(
        "Loss event #{} on {}\nStarted at {}",
        loss_count,
        target,
        chrono::Local::now().format("%H:%M:%S"),
    );

    // Call directly on the UI thread - toast notifications are non-blocking,
    // they just register with Windows and return immediately.
    // Spawning a thread here would initialize COM with the wrong threading
    // model, corrupting the window message pump and preventing focus.
    let _ = notify_rust::Notification::new()
        .summary("Network Monitor - Loss Detected")
        .body(&body)
        .timeout(notify_rust::Timeout::Milliseconds(5000))
        .show();
}
