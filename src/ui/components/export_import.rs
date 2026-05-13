//! # Export/Import UI component
//!
//! Handles manual export buttons, JSON import, auto-export on stop,
//! and export status display. File I/O runs after cloning and releasing
//! the shared state lock to avoid blocking the pinger.

use eframe::egui;
use std::time::Instant;

use crate::core::export;
use crate::core::import;
use crate::core::state::{SharedState, lock_state};

/// Export-related state
pub struct ExportState {
    pub export_path: String,
    pub auto_export_csv: bool,
    pub auto_export_json: bool,
    pub auto_export_isp: bool,
    pub auto_export_log: bool,
    pub status: Option<(String, Instant)>,
}

impl ExportState {
    pub fn new(export_path: String) -> Self {
        Self {
            export_path,
            auto_export_csv: false,
            auto_export_json: false,
            auto_export_isp: false,
            auto_export_log: false,
            status: None,
        }
    }

    pub fn from_saved(saved: &crate::core::config::SavedConfig) -> Self {
        Self {
            export_path: saved.export_path.clone(),
            auto_export_csv: saved.auto_export_csv,
            auto_export_json: saved.auto_export_json,
            auto_export_isp: saved.auto_export_isp,
            auto_export_log: saved.auto_export_log,
            status: None,
        }
    }
}

/// Render export buttons, import button, and status in the sidebar.
pub fn render(ui: &mut egui::Ui, state: &SharedState, exp: &mut ExportState) {
    ui.heading("📥 Export");
    ui.horizontal(|ui| {
        if ui.button("CSV").clicked() { do_export(state, exp, "csv"); }
        if ui.button("JSON").clicked() { do_export(state, exp, "json"); }
    });
    if ui.button("📋 ISP Report").on_hover_text("Human-readable report for your ISP").clicked() {
        do_export(state, exp, "txt");
    }
    if ui.button("🖥 Console Log").on_hover_text("Raw ping log output").clicked() {
        do_export(state, exp, "log");
    }

    ui.add_space(8.0);
    ui.separator();
    ui.heading("📂 Import");
    if ui.button("📥 Load JSON...").on_hover_text("Import a previous JSON export to review").clicked() {
        do_import(state, exp);
    }

    render_status(ui, exp);
}

/// Check if the pinger auto-stopped and there's a pending auto-export.
pub fn check_auto_export_pending(state: &SharedState, exp: &mut ExportState) {
    let pending = {
        let shared = lock_state(&state);
        shared.auto_export_pending
    };

    if pending {
        run_auto_export(state, exp);
        let mut shared = lock_state(&state);
        shared.auto_export_pending = false;
    }
}

/// Run auto-export after a manual or auto stop (if any formats are enabled).
pub fn run_auto_export(state: &SharedState, exp: &mut ExportState) {
    let any_enabled = exp.auto_export_csv || exp.auto_export_json
        || exp.auto_export_isp || exp.auto_export_log;
    if !any_enabled {
        return;
    }

    let snapshot = lock_state(state).clone();

    let msg = export::run_auto_export(
        &snapshot,
        &exp.export_path,
        exp.auto_export_csv,
        exp.auto_export_json,
        exp.auto_export_isp,
        exp.auto_export_log,
    );

    exp.status = Some((msg, Instant::now()));
}

fn do_export(state: &SharedState, exp: &mut ExportState, format: &str) {
    let snapshot = lock_state(state).clone();

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("network-monitor-{}.{}", timestamp, format);
    let export_dir = match export::export_dir(&exp.export_path) {
        Ok(dir) => dir,
        Err(err) => {
            exp.status = Some((format!("❌ {}", err), Instant::now()));
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
        Ok(()) => exp.status = Some((format!("✅ Saved {}", filename), Instant::now())),
        Err(err) => exp.status = Some((format!("❌ {}", err), Instant::now())),
    }
}

fn do_import(state: &SharedState, exp: &mut ExportState) {
    let file = rfd::FileDialog::new()
        .set_title("Import JSON export")
        .add_filter("JSON files", &["json"])
        .pick_file();

    let Some(path) = file else { return };

    match import::read_json(&path) {
        Ok(imported) => {
            let mut shared = lock_state(&state);
            shared.running = false;
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
            exp.status = Some((format!("✅ Imported {}", filename), Instant::now()));
        }
        Err(err) => {
            exp.status = Some((format!("❌ {}", err), Instant::now()));
        }
    }
}

fn render_status(ui: &mut egui::Ui, exp: &ExportState) {
    if let Some((text, when)) = &exp.status {
        if when.elapsed().as_secs() < 5 {
            ui.colored_label(egui::Color32::from_rgb(150, 200, 150), text);
        }
    }
}
