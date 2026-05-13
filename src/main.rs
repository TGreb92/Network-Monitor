//! # Network Monitor - Entry Point
//!
//! Loads config, creates shared state, launches the GUI.
//! Pinger threads are spawned after the window is created.

// Build as a Windows GUI app - no console window at all.
// Only in release builds so `cargo run` still shows a console for debugging.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod ui;

use core::config;
use core::state::{new_shared_state, PingConfig};

fn main() -> eframe::Result<()> {
    // SAFETY: CoInitializeEx is a Windows API that sets the COM threading model
    // for the current thread. We must call this before eframe starts because
    // notify-rust (toast notifications) and rfd (file dialogs) both use COM
    // internally. If COM is first initialized on a background thread with
    // COINIT_MULTITHREADED, it conflicts with the STA model that eframe/winit
    // needs for the window message pump, making the window unfocusable.
    // The call is safe: it only affects thread-local COM state and returns
    // an HRESULT that we intentionally ignore (S_FALSE if already initialized).
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Com::CoInitializeEx(
            std::ptr::null(),
            windows_sys::Win32::System::Com::COINIT_APARTMENTTHREADED as u32,
        );
    }

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
            // Do NOT spawn threads here - let the window render first.
            // Threads are spawned on the first frame via App::update().
            Ok(Box::new(ui::app::NetworkMonitorApp::new(state_clone, cc)))
        }),
    )
}
