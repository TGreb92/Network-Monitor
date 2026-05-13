//! # Notifications - Toast notification management
//!
//! Syncs notification config to shared state, checks for pending events,
//! and fires Windows toast notifications via notify-rust.
//! Supports 3 latency tiers (Elevated, High, Critical) plus loss events.

use eframe::egui;

use crate::core::state::{SharedState, PingThresholds, lock_state};

/// Notification preferences (persisted via config)
pub struct NotificationState {
    pub notify_on_loss: bool,
    pub notify_on_elevated_ping: bool,
    pub notify_on_high_ping: bool,
    pub notify_on_critical_ping: bool,
    pub threshold_elevated_ms: u32,
    pub threshold_high_ms: u32,
    pub threshold_critical_ms: u32,
    pub muted: bool,
}

impl NotificationState {
    pub fn new() -> Self {
        Self {
            notify_on_loss: false,
            notify_on_elevated_ping: false,
            notify_on_high_ping: false,
            notify_on_critical_ping: false,
            threshold_elevated_ms: 100,
            threshold_high_ms: 200,
            threshold_critical_ms: 500,
            muted: false,
        }
    }

    pub fn from_saved(saved: &crate::core::config::SavedConfig) -> Self {
        Self {
            notify_on_loss: saved.notify_on_loss,
            notify_on_elevated_ping: saved.notify_on_elevated_ping,
            notify_on_high_ping: saved.notify_on_high_ping,
            notify_on_critical_ping: saved.notify_on_critical_ping,
            threshold_elevated_ms: saved.threshold_elevated_ms,
            threshold_high_ms: saved.threshold_high_ms,
            threshold_critical_ms: saved.threshold_critical_ms,
            muted: false,
        }
    }
}

/// Sync notification config to shared state and fire any pending toasts.
/// Called once per frame from the sidebar render.
pub fn sync_and_fire(ctx: &egui::Context, state: &SharedState, notif: &NotificationState) {
    let mut shared = lock_state(&state);

    // Push config into shared state so the pinger can set pending flags
    shared.notify_loss_enabled = notif.notify_on_loss;
    shared.thresholds = PingThresholds {
        elevated_ms: notif.threshold_elevated_ms as f64,
        high_ms: notif.threshold_high_ms as f64,
        critical_ms: notif.threshold_critical_ms as f64,
    };
    shared.ping_tiers.notify_elevated_enabled = notif.notify_on_elevated_ping;
    shared.ping_tiers.notify_high_enabled = notif.notify_on_high_ping;
    shared.ping_tiers.notify_critical_enabled = notif.notify_on_critical_ping;

    if notif.muted {
        // Clear pending flags without firing
        shared.notify_loss_pending = false;
        shared.ping_tiers.notify_elevated_pending = false;
        shared.ping_tiers.notify_high_pending = false;
        shared.ping_tiers.notify_critical_pending = false;
        return;
    }

    // Collect pending events under the lock
    let mut toasts: Vec<(&str, String)> = Vec::new();
    let target = shared.config.target.clone();

    if shared.notify_loss_pending && shared.notify_loss_enabled {
        shared.notify_loss_pending = false;
        let count = shared.loss_tracker.count;
        toasts.push(("Loss Detected", format!(
            "Loss event #{} on {}\nStarted at {}",
            count, target, chrono::Local::now().format("%H:%M:%S"),
        )));
    }

    let thresholds = shared.thresholds.clone();
    for (label, count, threshold) in shared.ping_tiers.drain_pending(&thresholds) {
        toasts.push((label, format!(
            "{} #{} on {} (>= {}ms)\nDetected at {}",
            label, count, target, threshold,
            chrono::Local::now().format("%H:%M:%S"),
        )));
    }

    drop(shared);

    for (summary, body) in toasts {
        show_toast(ctx, summary, &body);
    }
}

/// Render the mute/unmute toggle for the sidebar
pub fn render_mute_toggle(ui: &mut egui::Ui, notif: &mut NotificationState) {
    ui.horizontal(|ui| {
        let label = if notif.muted { "🔇 Unmute" } else { "🔔 Mute" };
        if ui.button(label).on_hover_text("Mute/unmute all toast notifications").clicked() {
            notif.muted = !notif.muted;
        }
        if notif.muted {
            ui.colored_label(egui::Color32::from_rgb(255, 180, 50), "Muted");
        }
        if ui.button("🔔 Test").on_hover_text("Send a test toast notification").clicked() {
            show_toast_impl("Test", "This is a test notification");
        }
    });
}

/// Show a Windows toast notification (non-blocking, runs on UI thread)
fn show_toast(_ctx: &egui::Context, summary: &str, body: &str) {
    show_toast_impl(summary, body);
}

fn show_toast_impl(summary: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(&format!("Network Monitor - {}", summary))
        .body(body)
        .timeout(notify_rust::Timeout::Milliseconds(5000))
        .show();
}
