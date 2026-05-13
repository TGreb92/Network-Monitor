//! # System Tray Icon
//!
//! Creates a Windows system tray icon with hover tooltip and right-click menu.
//! Supports show/hide window, start/stop test, minimize to tray, and exit.
//!
//! Uses MenuEvent::set_event_handler because eframe/winit consumes the Windows
//! message loop, preventing MenuEvent::receiver() from receiving events.

use std::sync::{Arc, Mutex};
use eframe::egui;
use tray_icon::menu::{Menu, MenuItem, MenuId, PredefinedMenuItem, MenuEvent};
use tray_icon::{TrayIconBuilder, Icon};

use crate::core::state::{SharedState, PingTier, lock_state};

/// Hide the window from taskbar and screen using the native Windows API.
/// This keeps winit's event loop alive (unlike ViewportCommand::Visible(false)).
#[cfg(windows)]
pub fn hide_window(_ctx: &egui::Context) {
    unsafe {
        let title = "Network Monitor\0".encode_utf16().collect::<Vec<u16>>();
        let hwnd = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
            std::ptr::null(),
            title.as_ptr(),
        );
        if !hwnd.is_null() {
            windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                hwnd,
                windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE,
            );
        }
    }
}

/// Show and restore the window using the native Windows API.
#[cfg(windows)]
fn show_window_native() {
    unsafe {
        let title = "Network Monitor\0".encode_utf16().collect::<Vec<u16>>();
        let hwnd = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(
            std::ptr::null(),
            title.as_ptr(),
        );
        if !hwnd.is_null() {
            windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                hwnd,
                windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW,
            );
            windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow(hwnd);
        }
    }
}

/// Actions triggered by tray menu clicks
pub enum TrayAction {
    None,
    ShowWindow,
    ToggleTest,
    ToggleMute,
    MinimizeToTray,
    Exit,
}

/// Shared queue for menu events (handler writes, update reads)
static MENU_EVENTS: std::sync::LazyLock<Arc<Mutex<Vec<MenuId>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));

/// Egui context for requesting repaints from the event handler thread
static EGUI_CTX: std::sync::LazyLock<Arc<Mutex<Option<egui::Context>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(None)));

/// Menu ID for "Show Window" — handled directly in the event handler
/// because the eframe event loop stops when the window is hidden.
static SHOW_MENU_ID: std::sync::LazyLock<Arc<Mutex<Option<MenuId>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(None)));

/// Shared flag: set by handler when show is triggered from hidden state
static SHOW_REQUESTED: std::sync::LazyLock<Arc<std::sync::atomic::AtomicBool>> =
    std::sync::LazyLock::new(|| Arc::new(std::sync::atomic::AtomicBool::new(false)));

/// Holds the tray icon handle and menu item IDs.
pub struct TrayState {
    _icon: Option<tray_icon::TrayIcon>,
    last_tooltip: String,
    toggle_item: Option<MenuItem>,
    toggle_id: Option<MenuId>,
    mute_item: Option<MenuItem>,
    mute_id: Option<MenuId>,
    minimize_id: Option<MenuId>,
    exit_id: Option<MenuId>,
    /// Whether the window is currently hidden (minimized to tray)
    pub minimized_to_tray: bool,
}

impl TrayState {
    /// Create the tray icon with menu. Must be called on the event-loop thread.
    pub fn new() -> Self {
        // Install the global menu event handler (works with winit).
        // "Show Window" is handled directly here because when the window is
        // hidden, eframe's event loop stops and can't process queued events.
        let event_queue = MENU_EVENTS.clone();
        let ctx_ref = EGUI_CTX.clone();
        let show_id_ref = SHOW_MENU_ID.clone();
        let show_flag = SHOW_REQUESTED.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let id = event.id().clone();

            // If this is the "Show Window" action, restore immediately via Win32
            let is_show = show_id_ref.lock().ok()
                .and_then(|slot| slot.as_ref().map(|sid| *sid == id))
                .unwrap_or(false);

            if is_show {
                show_window_native();
                show_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                // Wake eframe so it clears minimized_to_tray
                if let Ok(ctx) = ctx_ref.lock() {
                    if let Some(ctx) = ctx.as_ref() {
                        ctx.request_repaint();
                    }
                }
                return;
            }

            if let Ok(mut queue) = event_queue.lock() {
                queue.push(id);
            }
            if let Ok(ctx) = ctx_ref.lock() {
                if let Some(ctx) = ctx.as_ref() {
                    ctx.request_repaint();
                }
            }
        }));

        // Double-click on tray icon restores the window (same as "Show Window")
        let show_flag2 = SHOW_REQUESTED.clone();
        let ctx_ref2 = EGUI_CTX.clone();
        tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
            if let tray_icon::TrayIconEvent::DoubleClick { .. } = event {
                show_window_native();
                show_flag2.store(true, std::sync::atomic::Ordering::Relaxed);
                if let Ok(ctx) = ctx_ref2.lock() {
                    if let Some(ctx) = ctx.as_ref() {
                        ctx.request_repaint();
                    }
                }
            }
        }));

        let (icon, show_id, toggle_item, toggle_id, mute_item, mute_id, minimize_id, exit_id) = build_tray_icon();

        // Store the show_id so the event handler can match it
        if let Some(id) = &show_id {
            if let Ok(mut slot) = SHOW_MENU_ID.lock() {
                *slot = Some(id.clone());
            }
        }

        Self {
            _icon: icon,
            last_tooltip: String::new(),
            toggle_item,
            toggle_id,
            mute_item,
            mute_id,
            minimize_id,
            exit_id,
            minimized_to_tray: false,
        }
    }

    /// Update the tooltip and check for menu events. Returns an action if clicked.
    pub fn update(&mut self, state: &SharedState, ctx: &egui::Context, muted: bool) -> TrayAction {
        // Store the context so the event handler can wake us up
        if let Ok(mut slot) = EGUI_CTX.lock() {
            *slot = Some(ctx.clone());
        }

        // Check if "Show Window" was handled directly by the event handler
        if SHOW_REQUESTED.load(std::sync::atomic::Ordering::Relaxed) {
            SHOW_REQUESTED.store(false, std::sync::atomic::Ordering::Relaxed);
            self.minimized_to_tray = false;
            return TrayAction::ShowWindow;
        }

        // Update tooltip
        if let Some(tray) = &self._icon {
            let tooltip = build_tooltip(state);
            if tooltip != self.last_tooltip {
                let _ = tray.set_tooltip(Some(&tooltip));
                self.last_tooltip = tooltip;
            }

            // Update Start/Stop label based on running state
            let running = lock_state(state).running;
            let label = if running { "Stop Test" } else { "Start Test" };
            if let Some(item) = &self.toggle_item {
                item.set_text(label);
            }

            // Update Mute/Unmute label
            let mute_label = if muted { "🔇 Unmute Notifications" } else { "🔔 Mute Notifications" };
            if let Some(item) = &self.mute_item {
                item.set_text(mute_label);
            }
        }

        // Drain all pending menu clicks
        let pending: Vec<MenuId> = MENU_EVENTS.lock().ok()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default();

        for id in pending {
            if self.toggle_id.as_ref() == Some(&id) {
                return TrayAction::ToggleTest;
            }
            if self.mute_id.as_ref() == Some(&id) {
                return TrayAction::ToggleMute;
            }
            if self.minimize_id.as_ref() == Some(&id) {
                return TrayAction::MinimizeToTray;
            }
            if self.exit_id.as_ref() == Some(&id) {
                return TrayAction::Exit;
            }
        }

        TrayAction::None
    }
}

fn build_tooltip(state: &SharedState) -> String {
    let shared = lock_state(state);

    let status = if shared.running { "Running" } else { "Stopped" };
    let loss = shared.packet_loss_pct();
    let avg = shared.avg_latency();
    let sent = shared.total_sent;

    let tier = if sent == 0 {
        "No data"
    } else {
        let recent_latency = shared.results.back().and_then(|r| r.latency_ms);
        PingTier::classify(recent_latency, &shared.thresholds).label()
    };

    if sent == 0 {
        format!("Network Monitor - {}", status)
    } else {
        format!(
            "Network Monitor - {}\nLoss: {:.1}% | Avg: {:.0}ms | {}",
            status, loss, avg, tier
        )
    }
}

type TrayBuildResult = (
    Option<tray_icon::TrayIcon>,
    Option<MenuId>,
    Option<MenuItem>,
    Option<MenuId>,
    Option<MenuItem>,
    Option<MenuId>,
    Option<MenuId>,
    Option<MenuId>,
);

fn build_tray_icon() -> TrayBuildResult {
    let icon_data = create_icon_rgba(16, 16, [100, 200, 100, 255]);
    let icon = match Icon::from_rgba(icon_data, 16, 16) {
        Ok(i) => i,
        Err(_) => return (None, None, None, None, None, None, None, None),
    };

    let show_item = MenuItem::new("Show Window", true, None);
    let toggle_item = MenuItem::new("Start Test", true, None);
    let mute_item = MenuItem::new("🔔 Mute Notifications", true, None);
    let minimize_item = MenuItem::new("Minimize to Tray", true, None);
    let exit_item = MenuItem::new("Exit", true, None);

    let show_id = Some(show_item.id().clone());
    let toggle_id = Some(toggle_item.id().clone());
    let mute_id = Some(mute_item.id().clone());
    let minimize_id = Some(minimize_item.id().clone());
    let exit_id = Some(exit_item.id().clone());

    let menu = Menu::new();
    let _ = menu.append(&show_item);
    let _ = menu.append(&toggle_item);
    let _ = menu.append(&mute_item);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&minimize_item);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&exit_item);

    let tray = TrayIconBuilder::new()
        .with_tooltip("Network Monitor")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .build()
        .ok();

    (tray, show_id, Some(toggle_item), toggle_id, Some(mute_item), mute_id, minimize_id, exit_id)
}

/// Create a simple filled circle icon as RGBA bytes
fn create_icon_rgba(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let radius = cx - 1.0;

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= radius {
                let idx = ((y * width + x) * 4) as usize;
                pixels[idx..idx + 4].copy_from_slice(&color);
            }
        }
    }
    pixels
}
