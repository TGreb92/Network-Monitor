//! # Network Monitor — Entry Point
//!
//! Loads config, creates shared state, launches the GUI.
//! Pinger threads are spawned after the window is created.

mod core;
mod ui;

use core::config;
use core::pinger;
use core::state::{new_shared_state, PingConfig};

fn main() -> eframe::Result<()> {
    #[cfg(windows)]
    unsafe {
        winapi::um::wincon::FreeConsole();
    }

    let saved = config::load();
    let ping_config = PingConfig::from_saved(&saved);
    let shared = new_shared_state(ping_config);

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
            let _pinger_handle = pinger::start_pinger(state_clone.clone());
            let _gw_pinger_handle = pinger::start_gateway_pinger(state_clone.clone());
            Ok(Box::new(ui::app::NetworkMonitorApp::new(state_clone, cc)))
        }),
    )
}
