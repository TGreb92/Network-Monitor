//! # Network Monitor — Entry Point
//!
//! Initializes the application: hides the console window (Windows only),
//! creates shared state, and launches the egui/eframe GUI.
//! Pinger threads are spawned after the window is created.

mod app;
mod export;
mod monitor;
mod pinger;
mod state;
mod ui_helpers;

use state::{new_shared_state, PingConfig};

fn main() -> eframe::Result<()> {
    // Hide the console window on Windows so the app runs as a pure GUI application.
    #[cfg(windows)]
    unsafe {
        winapi::um::wincon::FreeConsole();
    }

    let config = PingConfig::default();
    let shared = new_shared_state(config);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Network Monitor")
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };

    let state_clone = shared.clone();
    eframe::run_native(
        "Network Monitor",
        options,
        Box::new(move |cc| {
            // Spawn pinger threads AFTER the window is created,
            // so subprocess calls don't steal focus during startup.
            let _pinger_handle = pinger::start_pinger(state_clone.clone());
            let _gw_pinger_handle = pinger::start_gateway_pinger(state_clone.clone());
            Ok(Box::new(app::NetworkMonitorApp::new(state_clone, cc)))
        }),
    )
}
