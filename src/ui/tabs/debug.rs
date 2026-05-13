//! # Debug Tab - Testing and diagnostics
//!
//! Buttons for triggering test notifications, simulating latency tiers,
//! and inspecting internal state.

use eframe::egui;

use crate::core::state::{SharedState, PingState, lock_state};
use crate::ui::components::notifications;

/// Render the Debug tab contents
pub fn render(ui: &mut egui::Ui, state: &SharedState) {
    ui.heading("🔧 Debug Tools");
    ui.label("Use these to test notifications and simulate events without waiting for real network issues.");
    ui.add_space(8.0);

    render_toast_tests(ui);
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);
    render_simulate_events(ui, state);
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);
    render_state_inspector(ui, state);
}

fn render_toast_tests(ui: &mut egui::Ui) {
    ui.heading("🔔 Test Notifications");
    ui.label("Fire test toast notifications to verify they appear.");
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        if ui.button("Test Toast").clicked() {
            notifications::fire_toast("Test", "This is a test notification");
        }
        if ui.button("Loss Toast").clicked() {
            notifications::fire_toast("Loss Detected", "Simulated loss event #1\nTriggered from Debug tab");
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Elevated Toast").clicked() {
            notifications::fire_toast("Elevated Ping", "Simulated elevated ping (>= 100ms)");
        }
        if ui.button("High Toast").clicked() {
            notifications::fire_toast("High Ping", "Simulated high ping (>= 200ms)");
        }
        if ui.button("Critical Toast").clicked() {
            notifications::fire_toast("Critical Ping", "Simulated critical ping (>= 500ms)");
        }
    });
}

fn render_simulate_events(ui: &mut egui::Ui, state: &SharedState) {
    ui.heading("⚡ Simulate Events");
    ui.label("Inject fake ping results into the running state.");
    ui.add_space(4.0);

    let running = lock_state(state).running;
    if !running {
        ui.colored_label(egui::Color32::from_rgb(255, 180, 50), "Start a test first to simulate events.");
        return;
    }

    ui.horizontal(|ui| {
        if ui.button("Normal (15ms)").clicked() {
            inject_ping(state, Some(15.0), true);
        }
        if ui.button("Elevated (120ms)").clicked() {
            inject_ping(state, Some(120.0), true);
        }
        if ui.button("High (250ms)").clicked() {
            inject_ping(state, Some(250.0), true);
        }
        if ui.button("Critical (600ms)").clicked() {
            inject_ping(state, Some(600.0), true);
        }
        if ui.button("Timeout").clicked() {
            inject_ping(state, None, false);
        }
    });

    ui.add_space(4.0);
    ui.label("Gateway:");
    ui.horizontal(|ui| {
        if ui.button("GW OK (2ms)").clicked() {
            inject_gateway_ping(state, Some(2.0), true);
        }
        if ui.button("GW Slow (50ms)").clicked() {
            inject_gateway_ping(state, Some(50.0), true);
        }
        if ui.button("GW Timeout").clicked() {
            inject_gateway_ping(state, None, false);
        }
    });
}

fn inject_ping(state: &SharedState, latency_ms: Option<f64>, success: bool) {
    use crate::core::state::PingResult;

    let mut shared = lock_state(state);
    shared.seq_counter += 1;
    let seq = shared.seq_counter;
    let now = chrono::Local::now().naive_local();
    let elapsed_secs = shared.elapsed_secs();

    if success { shared.total_received += 1; }
    shared.total_sent += 1;

    let result = PingResult {
        seq,
        success,
        latency_ms,
        timestamp: now,
        elapsed_secs,
    };
    shared.push_result(result);

    let log_msg = if success {
        format!(
            "[{}] #{} [DEBUG] Simulated: time={}ms",
            now.format("%H:%M:%S"), seq,
            latency_ms.map(|l| format!("{:.0}", l)).unwrap_or("?".into())
        )
    } else {
        format!("[{}] #{} [DEBUG] Simulated timeout", now.format("%H:%M:%S"), seq)
    };
    shared.push_log(log_msg, latency_ms, success);
}

fn inject_gateway_ping(state: &SharedState, latency_ms: Option<f64>, success: bool) {
    let mut shared = lock_state(state);
    if !shared.gateway.enabled {
        shared.gateway.enabled = true;
        if shared.gateway.ip.is_none() {
            shared.gateway.ip = Some("192.168.0.1 (simulated)".into());
        }
    }
    shared.gateway.push_result(latency_ms, success);
}

fn render_state_inspector(ui: &mut egui::Ui, state: &SharedState) {
    ui.heading("📋 State Inspector");
    ui.add_space(4.0);

    let shared = lock_state(state);
    let info = snapshot_state_info(&shared);
    drop(shared);

    egui::Grid::new("debug_state_grid")
        .striped(true)
        .min_col_width(160.0)
        .show(ui, |ui| {
            for (label, value) in &info {
                ui.strong(*label);
                ui.label(value.as_str());
                ui.end_row();
            }
        });
}

fn snapshot_state_info(state: &PingState) -> Vec<(&'static str, String)> {
    vec![
        ("Running", state.running.to_string()),
        ("Total sent", state.total_sent.to_string()),
        ("Total received", state.total_received.to_string()),
        ("Packet loss %", format!("{:.2}%", state.packet_loss_pct())),
        ("Avg latency", format!("{:.1} ms", state.avg_latency())),
        ("Jitter avg", format!("{:.1} ms", state.jitter.avg())),
        ("Results buffered", state.results.len().to_string()),
        ("Log entries", state.log_entries.len().to_string()),
        ("Latencies buffered", state.all_latencies.len().to_string()),
        ("Interval reports", state.interval_reports.len().to_string()),
        ("Loss batches", state.loss_tracker.count.to_string()),
        ("Loss pending", state.notify_loss_pending.to_string()),
        ("Loss enabled", state.notify_loss_enabled.to_string()),
        ("Elevated batches", state.ping_tiers.count(crate::core::state::PingTier::Elevated).to_string()),
        ("High batches", state.ping_tiers.count(crate::core::state::PingTier::High).to_string()),
        ("Critical batches", state.ping_tiers.count(crate::core::state::PingTier::Critical).to_string()),
        ("Thresholds", format!(
            "E:{}ms H:{}ms C:{}ms",
            state.thresholds.elevated_ms as u32,
            state.thresholds.high_ms as u32,
            state.thresholds.critical_ms as u32,
        )),
        ("Gateway enabled", state.gateway.enabled.to_string()),
        ("Gateway IP", state.gateway.ip.as_deref().unwrap_or("-").to_string()),
        ("Config target", state.config.target.clone()),
    ]
}
