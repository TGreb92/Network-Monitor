//! # Notifications - Toast notification management
//!
//! Syncs notification config to shared state, checks for pending events,
//! and fires Windows toast notifications via notify-rust.
//! Supports 3 latency tiers (Elevated, High, Critical) plus loss events.
//! Enforces a cooldown between toasts to prevent notification spam.

use std::time::Instant;
use eframe::egui;

use crate::core::state::{SharedState, PingThresholds, PingTier, lock_state};

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
    /// Severity of the last toast (higher can override cooldown)
    last_toast_severity: PingTier,
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
            last_toast_severity: PingTier::Normal,
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
            last_toast_severity: PingTier::Normal,
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
    shared.ping_tiers.set_enabled(PingTier::Elevated, notif.notify_on_elevated_ping);
    shared.ping_tiers.set_enabled(PingTier::High, notif.notify_on_high_ping);
    shared.ping_tiers.set_enabled(PingTier::Critical, notif.notify_on_critical_ping);

    if notif.muted {
        shared.notify_loss_pending = false;
        shared.ping_tiers.reset_pending();
        return;
    }

    // Check if cooldown has expired - reset severity tracking if so
    let in_cooldown = notif.last_toast_time
        .is_some_and(|t| t.elapsed().as_secs_f64() < TOAST_COOLDOWN_SECS);
    if !in_cooldown {
        notif.last_toast_severity = PingTier::Normal;
    }

    // Determine the highest-severity pending event (peek without clearing)
    let target = shared.config.target.clone();
    let thresholds = shared.thresholds.clone();

    let mut severity = PingTier::Normal;
    let mut toast: Option<(String, String)> = None;

    // Check tier notifications (highest first)
    if let Some(tier) = shared.ping_tiers.highest_pending() {
        if tier > severity {
            severity = tier;
            let count = shared.ping_tiers.count(tier);
            let threshold = tier.threshold(&thresholds) as u32;
            toast = Some((tier.label().to_string(), format!(
                "{} #{} on {} (>= {}ms)\nDetected at {}",
                tier.label(), count, target, threshold,
                chrono::Local::now().format("%H:%M:%S"),
            )));
        }
    }

    // Loss is highest priority
    if shared.notify_loss_pending && shared.notify_loss_enabled {
        severity = PingTier::Loss;
        let count = shared.loss_tracker.count;
        toast = Some(("Loss Detected".to_string(), format!(
            "Loss event #{} on {}\nStarted at {}",
            count, target, chrono::Local::now().format("%H:%M:%S"),
        )));
    }

    // Fire if: cooldown expired, OR this is higher severity than the last toast
    let should_fire = severity > PingTier::Normal
        && (!in_cooldown || severity > notif.last_toast_severity);

    if should_fire {
        // NOW clear the consumed flags
        if severity == PingTier::Loss {
            shared.notify_loss_pending = false;
        }
        shared.ping_tiers.clear_pending_up_to(severity);
    }

    drop(shared);

    if let Some((summary, body)) = toast.filter(|_| should_fire) {
        show_toast(ctx, &summary, &body);
        notif.last_toast_time = Some(Instant::now());
        notif.last_toast_severity = severity;
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

/// Show a Windows toast notification.
fn show_toast(_ctx: &egui::Context, summary: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(&format!("Network Monitor - {}", summary))
        .body(body)
        .timeout(notify_rust::Timeout::Milliseconds(5000))
        .show();
}

/// Fire a toast notification directly. Only available in debug builds.
#[cfg(debug_assertions)]
pub fn fire_toast(summary: &str, body: &str) {
    let _ = notify_rust::Notification::new()
        .summary(&format!("Network Monitor - {}", summary))
        .body(body)
        .timeout(notify_rust::Timeout::Milliseconds(5000))
        .show();
}
