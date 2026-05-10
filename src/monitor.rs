//! # Monitor Tab — Data visualization
//!
//! Renders the Monitor tab: external stats, gateway health, latency chart,
//! and interval reports table.

use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};

use crate::state::PingState;
use crate::ui_helpers::{stat_card, loss_color, latency_color, jitter_color};

/// Render the full Monitor tab contents
pub fn render(ui: &mut egui::Ui, state: &PingState) {
    render_external_stats(ui, state);

    if state.gateway_enabled && state.gateway_ip.is_some() {
        render_gateway_stats(ui, state);
    }

    ui.add_space(8.0);
    render_latency_chart(ui, state);
    ui.add_space(8.0);
    render_interval_reports(ui, state);
}

/// Stat cards row for the external target: loss, latency, jitter, verdict
fn render_external_stats(ui: &mut egui::Ui, state: &PingState) {
    ui.horizontal_wrapped(|ui| {
        let loss = state.packet_loss_pct();
        let avg = state.avg_latency();
        let min_lat = state.min_latency();
        let max_lat = state.max_latency();
        let jitter = state.avg_jitter();

        stat_card(ui, "Packet Loss", &format!("{:.1}%", loss), loss_color(loss));
        stat_card(ui, "Avg Latency", &format!("{:.1} ms", avg), latency_color(avg));
        stat_card(
            ui, "Min Latency",
            &format!("{:.1} ms", if min_lat == f64::MAX { 0.0 } else { min_lat }),
            egui::Color32::from_rgb(150, 200, 255),
        );
        stat_card(ui, "Max Latency", &format!("{:.1} ms", max_lat), latency_color(max_lat));
        stat_card(ui, "Jitter", &format!("{:.1} ms", jitter), jitter_color(jitter));

        let (verdict, color) = connection_verdict(loss, avg, state.total_sent);
        stat_card(ui, "Connection", verdict, color);
    });
}

/// Gateway stats row with local-vs-ISP diagnosis
fn render_gateway_stats(ui: &mut egui::Ui, state: &PingState) {
    ui.add_space(4.0);
    let gw_ip = state.gateway_ip.as_deref().unwrap_or("?");
    ui.heading(format!("🌐 Gateway: {}", gw_ip));
    ui.horizontal_wrapped(|ui| {
        let gw_loss = state.gw_packet_loss_pct();
        let gw_avg = state.gw_avg_latency();
        let gw_jitter = state.gw_avg_jitter();
        let ext_loss = state.packet_loss_pct();

        stat_card(ui, "GW Loss", &format!("{:.1}%", gw_loss), loss_color(gw_loss));
        stat_card(ui, "GW Avg", &format!("{:.1} ms", gw_avg), latency_color(gw_avg));
        stat_card(ui, "GW Jitter", &format!("{:.1} ms", gw_jitter), jitter_color(gw_jitter));

        let (diag, color) = network_diagnosis(state.gw_total_sent, state.total_sent, gw_loss, ext_loss);
        stat_card(ui, "Diagnosis", diag, color);
    });
}

/// Latency-over-time chart with optional gateway overlay
fn render_latency_chart(ui: &mut egui::Ui, state: &PingState) {
    ui.heading("Latency Over Time");

    let ext_line = Line::new(latency_to_plot_points(&state.all_latencies))
        .color(egui::Color32::from_rgb(100, 200, 255))
        .name("External (ms)");

    let gw_line = Line::new(latency_to_plot_points(&state.gw_all_latencies))
        .color(egui::Color32::from_rgb(255, 200, 100))
        .name("Gateway (ms)");

    let show_gateway = state.gateway_enabled && state.gateway_ip.is_some();

    Plot::new("latency_plot")
        .height(180.0)
        .allow_drag(false)
        .allow_zoom(false)
        .show_axes(true)
        .y_axis_label("ms")
        .legend(egui_plot::Legend::default())
        .show(ui, |plot_ui| {
            plot_ui.line(ext_line);
            if show_gateway { plot_ui.line(gw_line); }
        });
}

/// Scrollable interval reports table (newest first)
fn render_interval_reports(ui: &mut egui::Ui, state: &PingState) {
    if state.interval_reports.is_empty() { return; }

    ui.heading("Interval Reports");
    egui::ScrollArea::vertical()
        .max_height(200.0)
        .show(ui, |ui| {
            egui::Grid::new("reports_grid")
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    for label in ["Time", "Pings", "OK", "Fail", "Loss%", "Avg ms", "Min ms", "Max ms"] {
                        ui.strong(label);
                    }
                    ui.end_row();

                    for report in state.interval_reports.iter().rev() {
                        ui.label(format!("{}-{}", report.start_time.format("%H:%M:%S"), report.end_time.format("%H:%M:%S")));
                        ui.label(report.total_pings.to_string());
                        ui.label(report.successful.to_string());
                        ui.label(report.failed.to_string());
                        ui.colored_label(loss_color(report.packet_loss_pct), format!("{:.1}", report.packet_loss_pct));
                        ui.label(format!("{:.1}", report.avg_latency_ms));
                        ui.label(format!("{:.1}", report.min_latency_ms));
                        ui.label(format!("{:.1}", report.max_latency_ms));
                        ui.end_row();
                    }
                });
        });
}

/// Convert a VecDeque of latencies into plot points [index, value]
fn latency_to_plot_points(latencies: &std::collections::VecDeque<f64>) -> PlotPoints<'_> {
    PlotPoints::new(
        latencies.iter().enumerate().map(|(i, &lat)| [i as f64, lat]).collect()
    )
}

/// Determine connection quality verdict from loss and latency
fn connection_verdict(loss: f64, avg: f64, total_sent: u64) -> (&'static str, egui::Color32) {
    if total_sent == 0 {
        ("No data", egui::Color32::GRAY)
    } else if loss < 1.0 && avg < 50.0 {
        ("Excellent", egui::Color32::from_rgb(0, 255, 100))
    } else if loss < 5.0 && avg < 100.0 {
        ("Good", egui::Color32::from_rgb(150, 255, 50))
    } else if loss < 15.0 {
        ("Fair", egui::Color32::from_rgb(255, 200, 50))
    } else {
        ("Poor", egui::Color32::from_rgb(255, 80, 80))
    }
}

/// Diagnose whether network issues are local (gateway) or external (ISP)
fn network_diagnosis(gw_sent: u64, ext_sent: u64, gw_loss: f64, ext_loss: f64) -> (&'static str, egui::Color32) {
    if gw_sent == 0 || ext_sent == 0 {
        ("Collecting...", egui::Color32::GRAY)
    } else if gw_loss > 2.0 {
        ("⚠ Local network issue", egui::Color32::from_rgb(255, 150, 50))
    } else if ext_loss > 2.0 {
        ("ISP/route issue", egui::Color32::from_rgb(255, 200, 50))
    } else {
        ("✅ All clear", egui::Color32::from_rgb(100, 255, 100))
    }
}
