//! # Notifications - Toast notification management
//!
//! Syncs notification config to shared state, checks for pending events,
//! and fires Windows toast notifications via notify-rust.
//! Supports 3 latency tiers (Elevated, High, Critical) plus loss events.
//! Enforces a cooldown between toasts to prevent notification spam.

use std::time::Instant;
use eframe::egui;

use crate::core::state::{SharedState, PingThresholds, lock_state};

/// Minimum seconds between consecutive toast notifications
const TOAST_COOLDOWN_SECS: f64 = 5.0;

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
    /// Last time a toast was fired (for cooldown)
    last_toast_time: Option<Instant>,
    /// Severity of the last toast (higher = more severe, used for cooldown override)
    last_toast_severity: u8,
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
            last_toast_time: None,
            last_toast_severity: 0,
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
            last_toast_time: None,
            last_toast_severity: 0,
        }
    }
}

/// Sync notification config to shared state and fire any pending toasts.
/// Called once per frame from the sidebar render.
pub fn sync_and_fire(ctx: &egui::Context, state: &SharedState, notif: &mut NotificationState) {
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
        shared.notify_loss_pending = false;
        shared.ping_tiers.notify_elevated_pending = false;
        shared.ping_tiers.notify_high_pending = false;
        shared.ping_tiers.notify_critical_pending = false;
        return;
    }

    // Determine the highest-severity pending event
    // Severity: 0=none, 1=Elevated, 2=High, 3=Critical, 4=Loss
    let mut toast: Option<(&str, String)> = None;
    let mut severity: u8 = 0;
    let target = shared.config.target.clone();

    let thresholds = shared.thresholds.clone();
    let tier_pending = shared.ping_tiers.drain_pending(&thresholds);
    for (label, count, threshold) in tier_pending {
        let s = match label {
            "Elevated Ping" => 1,
            "High Ping" => 2,
            "Critical Ping" => 3,
            _ => 1,
        };
        if s > severity {
            severity = s;
            toast = Some((label, format!(
                "{} #{} on {} (>= {}ms)\nDetected at {}",
                label, count, target, threshold,
                chrono::Local::now().format("%H:%M:%S"),
            )));
        }
    }

    if shared.notify_loss_pending && shared.notify_loss_enabled {
        shared.notify_loss_pending = false;
        severity = 4;
        let count = shared.loss_tracker.count;
        toast = Some(("Loss Detected", format!(
            "Loss event #{} on {}\nStarted at {}",
            count, target, chrono::Local::now().format("%H:%M:%S"),
        )));
    }

    drop(shared);

    // Fire if: no cooldown active, OR new severity is higher than the last toast
    let in_cooldown = notif.last_toast_time
        .is_some_and(|t| t.elapsed().as_secs_f64() < TOAST_COOLDOWN_SECS);
    let can_override = severity > notif.last_toast_severity;

    if let Some((summary, body)) = toast {
        if !in_cooldown || can_override {
            show_toast(ctx, summary, &body);
            notif.last_toast_time = Some(Instant::now());
            notif.last_toast_severity = severity;
        }
    }

    // Reset severity tracking after cooldown expires
    if !in_cooldown {
        notif.last_toast_severity = 0;
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
    });
}

/// Show a Windows toast notification (non-blocking, runs on UI thread)
fn show_toast(_ctx: &egui::Context, summary: &str, body: &str) {
    fire_toast(summary, body);
}

/// Fire a toast notification directly. Public for use by the Debug tab.
pub fn fire_toast(summary: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(&format!("Network Monitor - {}", summary))
        .body(body)
        .timeout(notify_rust::Timeout::Milliseconds(5000))
        .show();
}
