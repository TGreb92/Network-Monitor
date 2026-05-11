//! # Network Monitor — Entry Point
//!
//! Loads config, creates shared state, launches the GUI.
//! Pinger threads are spawned after the window is created.

// Build as a Windows GUI app — no console window at all.
// Only in release builds so `cargo run` still shows a console for debugging.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod ui;

use core::config;
use core::state::{new_shared_state, PingConfig};

fn main() -> eframe::Result<()> {
    let saved = config::load();
    let ping_config = PingConfig::from_saved(&saved);
    let shared = new_shared_state(ping_config);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Network Monitor")
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([600.0, 400.0])
            .with_active(true),
        ..Default::default()
    };

    let state_clone = shared.clone();
    eframe::run_native(
        "Network Monitor",
        options,
        Box::new(move |cc| {
            // Do NOT spawn threads here — let the window render first.
            // Threads are spawned on the first frame via App::update().
            Ok(Box::new(ui::app::NetworkMonitorApp::new(state_clone, cc)))
        }),
    )
}
