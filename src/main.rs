//! # Network Monitor — Entry Point
//!
//! Initializes the application: hides the console window (Windows only),
//! creates shared state, spawns the background pinger thread, and launches
//! the egui/eframe GUI.

mod app;
mod pinger;
mod state;

use state::{new_shared_state, PingConfig};

fn main() {
    // Hide the console window on Windows so the app runs as a pure GUI application.
    #[cfg(windows)]
    unsafe {
        winapi::um::wincon::FreeConsole();
    }

    // Initialize shared state with default config (target: 8.8.8.8, timeout: 2000ms)
    let config = PingConfig::default();
    let shared = new_shared_state(config);

    // Spawn the background pinger thread. It continuously pings the target
    // at ~1-second intervals and pushes results into shared state.
    // The handle is kept alive (_pinger_handle) so the thread isn't dropped.
    let _pinger_handle = pinger::start_pinger(shared.clone());

    // Configure the native window: title, default size, and minimum size
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Network Monitor")
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };

    // Launch the eframe event loop. This blocks until the window is closed.
    // The closure creates our PingMonitorApp with a clone of the shared state.
    let state_clone = shared.clone();
    eframe::run_native(
        "Network Monitor",
        options,
        Box::new(move |cc| Ok(Box::new(app::NetworkMonitorApp::new(state_clone, cc)))),
    )
    .expect("Failed to run eframe");
}
