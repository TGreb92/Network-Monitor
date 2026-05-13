//! # Monitor Tab - Data visualization
//!
//! Renders the Monitor tab: external stats, gateway health, latency chart
//! with selectable time window, and interval reports table.

use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};

use crate::core::state::{PingState, PingTier};
use crate::ui::components::helpers::{stat_card, loss_color, latency_color, jitter_color};

/// Time window options for the chart (in seconds, 0 = show all)
const TIME_WINDOWS: &[(f64, &str)] = &[
    (60.0, "1m"),
    (300.0, "5m"),
    (900.0, "15m"),
    (1800.0, "30m"),
    (0.0, "All"),
];

/// Persistent state for the monitor tab
pub struct MonitorState {
    pub selected_window: usize,
}

impl MonitorState {
    pub fn new() -> Self {
        Self { selected_window: 1 } // default to 5m
    }
}

/// Render the full Monitor tab contents
pub fn render(ui: &mut egui::Ui, state: &PingState, monitor: &mut MonitorState) {
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            render_external_stats(ui, state);

            if state.gateway.enabled && state.gateway.ip.is_some() {
                render_gateway_stats(ui, state);
            }

            ui.add_space(8.0);
            render_latency_chart(ui, state, monitor);
            ui.add_space(8.0);
            render_interval_reports(ui, state);
        });
}

/// Stat cards row for the external target: loss, latency, jitter, verdict
fn render_external_stats(ui: &mut egui::Ui, state: &PingState) {
    ui.horizontal_wrapped(|ui| {
        let loss = state.packet_loss_pct();
        let avg = state.avg_latency();
        let min_lat = state.min_latency();
        let max_lat = state.max_latency();
        let jitter = state.jitter.avg();

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
    let gw_ip = state.gateway.ip.as_deref().unwrap_or("?");
    ui.heading(format!("🌐 Gateway: {}", gw_ip));
    ui.horizontal_wrapped(|ui| {
        let gw_loss = state.gateway.packet_loss_pct();
        let gw_avg = state.gateway.avg_latency();
        let gw_jitter = state.gateway.jitter.avg();
        let ext_loss = state.packet_loss_pct();

        stat_card(ui, "GW Loss", &format!("{:.1}%", gw_loss), loss_color(gw_loss));
        stat_card(ui, "GW Avg", &format!("{:.1} ms", gw_avg), latency_color(gw_avg));
        stat_card(ui, "GW Jitter", &format!("{:.1} ms", gw_jitter), jitter_color(gw_jitter));

        let (diag, color) = network_diagnosis(state.gateway.total_sent, state.total_sent, gw_loss, ext_loss);
        stat_card(ui, "Diagnosis", diag, color);
    });
}

/// Latency-over-time chart with time window selector, timeout markers, and gateway overlay.
fn render_latency_chart(ui: &mut egui::Ui, state: &PingState, monitor: &mut MonitorState) {
    render_window_selector(ui, monitor);

    let elapsed = state.elapsed_secs();
    let window_secs = TIME_WINDOWS[monitor.selected_window].0;
    let min_time = if window_secs > 0.0 { (elapsed - window_secs).max(0.0) } else { 0.0 };

    let gw_line = build_gateway_chart_data(state, window_secs);
    let visible_results = build_tooltip_data(state, min_time);
    let show_gateway = state.gateway.enabled && state.gateway.ip.is_some();

    let x_fmt = |mark: egui_plot::GridMark, _range: &std::ops::RangeInclusive<f64>| {
        let total_secs = mark.value as u64;
        format!("{}:{:02}", total_secs / 60, total_secs % 60)
    };

    let mut plot = Plot::new("latency_plot")
        .height(200.0)
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .allow_boxed_zoom(false)
        .show_axes(true)
        .x_axis_label("Time (m:ss)")
        .y_axis_label("ms")
        .x_axis_formatter(x_fmt)
        .label_formatter(move |_name, value| format_nearest_tooltip(&visible_results, value.x))
        .legend(egui_plot::Legend::default());

    // Lock the X-axis to always show the full window width
    if window_secs > 0.0 {
        let max_time = elapsed.max(window_secs);
        plot = plot.include_x(max_time - window_secs).include_x(max_time);
    } else if elapsed > 0.0 {
        plot = plot.include_x(0.0).include_x(elapsed);
    }

    let thresholds = &state.thresholds;
    let (ext_line, timeout_markers, elevated_pts, high_pts, critical_pts) =
        build_external_chart_data(state, min_time);

    // Compute the max visible latency to auto-scale threshold lines.
    // Only show threshold lines that are within 2x of the max data point,
    // so a 500ms Critical line doesn't stretch the chart when pings are 20ms.
    let max_data_latency = state.results.iter()
        .filter(|r| r.elapsed_secs >= min_time)
        .filter_map(|r| r.latency_ms)
        .fold(0.0_f64, f64::max);
    let y_ceiling = (max_data_latency * 2.0).max(50.0);

    plot.show(ui, |plot_ui| {
        // Threshold lines - only show when data approaches them
        for &tier in &[PingTier::Elevated, PingTier::High, PingTier::Critical] {
            let thresh = tier.threshold(thresholds);
            if thresh > 0.0 && thresh <= y_ceiling {
                let [r, g, b] = tier.rgb();
                plot_ui.hline(egui_plot::HLine::new(thresh)
                    .color(egui::Color32::from_rgb(r, g, b))
                    .style(egui_plot::LineStyle::dashed_dense())
                    .name(format!("{} ({}ms)", tier.label(), thresh as u32)));
            }
        }

        plot_ui.line(ext_line);
        plot_ui.points(timeout_markers);
        // Tier dots - no legend name to avoid duplicating the threshold line entry
        for (tier, pts) in [
            (PingTier::Elevated, elevated_pts),
            (PingTier::High, high_pts),
            (PingTier::Critical, critical_pts),
        ] {
            if !pts.is_empty() {
                let [r, g, b] = tier.rgb();
                plot_ui.points(egui_plot::Points::new(pts)
                    .color(egui::Color32::from_rgb(r, g, b))
                    .radius(3.5));
            }
        }
        if show_gateway { plot_ui.line(gw_line); }
    });
}

fn render_window_selector(ui: &mut egui::Ui, monitor: &mut MonitorState) {
    ui.horizontal(|ui| {
        ui.heading("Latency Over Time");
        ui.add_space(16.0);
        for (idx, (_secs, label)) in TIME_WINDOWS.iter().enumerate() {
            if ui.selectable_label(monitor.selected_window == idx, *label).clicked() {
                monitor.selected_window = idx;
            }
        }
    });
}

fn build_external_chart_data(
    state: &PingState, min_time: f64
) -> (Line<'_>, egui_plot::Points<'_>, Vec<[f64; 2]>, Vec<[f64; 2]>, Vec<[f64; 2]>) {
    let thresholds = &state.thresholds;

    let latency_points: Vec<[f64; 2]> = state.results
        .iter()
        .filter(|r| r.elapsed_secs >= min_time)
        .filter_map(|r| r.latency_ms.map(|ms| [r.elapsed_secs, ms]))
        .collect();
    let ext_line = Line::new(PlotPoints::new(latency_points))
        .color(egui::Color32::from_rgb(100, 200, 255))
        .name("Latency (ms)");

    let timeout_points: Vec<[f64; 2]> = state.results
        .iter()
        .filter(|r| !r.success && r.elapsed_secs >= min_time)
        .map(|r| [r.elapsed_secs, 0.0])
        .collect();
    let timeout_markers = egui_plot::Points::new(timeout_points)
        .color(egui::Color32::RED)
        .radius(3.0)
        .name("Timeout");

    let mut elevated_pts = Vec::new();
    let mut high_pts = Vec::new();
    let mut critical_pts = Vec::new();

    for r in state.results.iter().filter(|r| r.elapsed_secs >= min_time) {
        match PingTier::classify(r.latency_ms, thresholds) {
            PingTier::Critical => critical_pts.push([r.elapsed_secs, r.latency_ms.unwrap()]),
            PingTier::High => high_pts.push([r.elapsed_secs, r.latency_ms.unwrap()]),
            PingTier::Elevated => elevated_pts.push([r.elapsed_secs, r.latency_ms.unwrap()]),
            PingTier::Normal | PingTier::Loss => {}
        }
    }

    (ext_line, timeout_markers, elevated_pts, high_pts, critical_pts)
}

/// Build gateway chart data, showing only the last `window_secs` worth of points.
/// Gateway latencies don't have timestamps, so we estimate based on count.
fn build_gateway_chart_data(state: &PingState, window_secs: f64) -> Line<'_> {
    let total = state.gateway.all_latencies.len();
    let skip_count = if window_secs > 0.0 && total > window_secs as usize {
        total - window_secs as usize
    } else {
        0
    };

    let points: Vec<[f64; 2]> = state.gateway.all_latencies
        .iter()
        .enumerate()
        .skip(skip_count)
        .map(|(idx, &lat)| [idx as f64, lat])
        .collect();

    Line::new(PlotPoints::new(points))
        .color(egui::Color32::from_rgb(255, 200, 100))
        .name("Gateway (ms)")
}

fn build_tooltip_data(state: &PingState, min_time: f64) -> Vec<(f64, Option<f64>, bool)> {
    state.results
        .iter()
        .filter(|result| result.elapsed_secs >= min_time)
        .map(|result| (result.elapsed_secs, result.latency_ms, result.success))
        .collect()
}

fn format_nearest_tooltip(visible_results: &[(f64, Option<f64>, bool)], cursor_time: f64) -> String {
    let nearest = visible_results.iter().min_by(|point_a, point_b| {
        let dist_a = (point_a.0 - cursor_time).abs();
        let dist_b = (point_b.0 - cursor_time).abs();
        dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
    });

    let Some((elapsed, latency, success)) = nearest else {
        return "No data".to_string();
    };
    let total_secs = *elapsed as u64;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    if *success {
        format!("⏱ {}:{:02}\n📶 {:.1} ms", mins, secs, latency.unwrap_or(0.0))
    } else {
        format!("⏱ {}:{:02}\n❌ Timeout", mins, secs)
    }
}

/// Scrollable interval reports table (newest first)
fn render_interval_reports(ui: &mut egui::Ui, state: &PingState) {
    if state.interval_reports.is_empty() { return; }

    ui.heading("Interval Reports");
    let available_height = ui.available_height() - 8.0;
    egui::ScrollArea::vertical()
        .max_height(available_height.max(60.0))
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            egui::Grid::new("reports_grid")
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    for label in ["Time", "Pings", "OK", "Fail", "Loss%", "Events", "Avg ms", "Min ms", "Max ms"] {
                        ui.strong(label);
                    }
                    ui.end_row();

                    for report in state.interval_reports.iter().rev() {
                        ui.label(format!("{}-{}", report.start_time.format("%H:%M:%S"), report.end_time.format("%H:%M:%S")));
                        ui.label(report.total_pings.to_string());
                        ui.label(report.successful.to_string());
                        ui.label(report.failed.to_string());
                        ui.colored_label(loss_color(report.packet_loss_pct), format!("{:.1}", report.packet_loss_pct));
                        ui.label(report.loss_events.to_string());
                        ui.label(format!("{:.1}", report.avg_latency_ms));
                        ui.label(format!("{:.1}", report.min_latency_ms));
                        ui.label(format!("{:.1}", report.max_latency_ms));
                        ui.end_row();
                    }
                });
        });
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
