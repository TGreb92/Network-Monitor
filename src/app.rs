//! # Network Monitor — GUI Layer
//!
//! Implements the main application window with two tabs:
//! - **Monitor**: Live stats dashboard with latency chart and interval reports
//! - **Console**: Scrolling ping log with color-coded success/failure entries
//!
//! A left sidebar provides configuration controls and start/stop functionality.
//! The GUI reads shared state via `RwLock` at ~2 FPS (500ms repaint interval)
//! to minimize CPU usage while staying responsive.

use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};

use crate::state::{PingConfig, SharedState};

/// Which tab is currently active in the main panel
#[derive(PartialEq)]
enum Tab {
    Monitor,
    Console,
}

/// Main application struct holding shared state and UI-local state
pub struct NetworkMonitorApp {
    /// Thread-safe shared state (read by GUI, written by pinger)
    state: SharedState,
    /// Currently selected tab
    active_tab: Tab,
    /// Whether the console log auto-scrolls to the latest entry
    auto_scroll: bool,
    /// Local copy of config fields for the sidebar text inputs.
    /// These are synced to shared state only when "Apply Config" is clicked.
    config_target: String,
    config_timeout: String,
    config_interval: String,
}

impl NetworkMonitorApp {
    /// Initialize the app, reading current config from shared state to populate
    /// the sidebar text fields.
    pub fn new(state: SharedState, _cc: &eframe::CreationContext<'_>) -> Self {
        let (target, timeout, interval) = {
            let s = state.read().unwrap_or_else(|e| e.into_inner());
            (
                s.config.target.clone(),
                s.config.timeout_ms.to_string(),
                s.config.interval_secs.to_string(),
            )
        };

        Self {
            state,
            active_tab: Tab::Monitor,
            auto_scroll: true,
            config_target: target,
            config_timeout: timeout,
            config_interval: interval,
        }
    }
}

impl eframe::App for NetworkMonitorApp {
    /// Called by eframe each frame. Schedules the next repaint after 500ms
    /// to keep CPU usage low while displaying near-real-time data.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        self.render_sidebar(ctx);
        self.render_main(ctx);
    }
}

impl NetworkMonitorApp {
    /// Render the left sidebar with config inputs, start/stop button, and quick stats
    fn render_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("config_panel")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.heading("⚙ Configuration");
                ui.separator();

                // Editable config fields — changes are buffered locally until Apply is clicked
                ui.label("Target host:");
                ui.text_edit_singleline(&mut self.config_target);
                ui.add_space(4.0);

                ui.label("Timeout (ms):");
                ui.text_edit_singleline(&mut self.config_timeout);
                ui.add_space(4.0);

                ui.label("Report interval (s):");
                ui.text_edit_singleline(&mut self.config_interval);
                ui.add_space(8.0);

                // Push local config edits into shared state
                if ui.button("✅ Apply Config").clicked() {
                    self.apply_config();
                }

                ui.separator();

                // Start/Stop toggle — reads and writes the `running` flag
                let running = {
                    let s = self.state.read().unwrap_or_else(|e| e.into_inner());
                    s.running
                };

                if running {
                    if ui.button("⏹ Stop").clicked() {
                        let mut s = self.state.write().unwrap_or_else(|e| e.into_inner());
                        s.running = false;
                    }
                    ui.colored_label(egui::Color32::from_rgb(100, 255, 100), "● RUNNING");
                } else {
                    if ui.button("▶ Start").clicked() {
                        let mut s = self.state.write().unwrap_or_else(|e| e.into_inner());
                        s.running = true;
                    }
                    ui.colored_label(egui::Color32::from_rgb(255, 100, 100), "● STOPPED");
                }

                ui.separator();

                // Quick stats summary in the sidebar
                let (sent, recv, loss) = {
                    let s = self.state.read().unwrap_or_else(|e| e.into_inner());
                    (s.total_sent, s.total_received, s.packet_loss_pct())
                };
                ui.label(format!("Sent: {}", sent));
                ui.label(format!("Received: {}", recv));
                ui.label(format!("Loss: {:.1}%", loss));
            });
    }

    /// Render the main central panel with tab selector and active tab content
    fn render_main(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Tab bar at the top
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Monitor, "📊 Monitor");
                ui.selectable_value(&mut self.active_tab, Tab::Console, "🖥 Console");
            });
            ui.separator();

            match self.active_tab {
                Tab::Monitor => self.render_monitor(ui),
                Tab::Console => self.render_console(ui),
            }
        });
    }

    /// Render the Monitor tab: stat cards, latency chart, and interval reports table.
    /// Holds a read lock for the duration of rendering to get a consistent snapshot.
    fn render_monitor(&mut self, ui: &mut egui::Ui) {
        let s = self.state.read().unwrap_or_else(|e| e.into_inner());

        // --- Stat Cards Row ---
        // Displays key metrics as colored cards with adaptive coloring
        ui.horizontal_wrapped(|ui| {
            let loss = s.packet_loss_pct();
            let avg = s.avg_latency();
            let min_lat = s.min_latency();
            let max_lat = s.max_latency();

            stat_card(ui, "Packet Loss", &format!("{:.1}%", loss), loss_color(loss));
            stat_card(
                ui,
                "Avg Latency",
                &format!("{:.1} ms", avg),
                latency_color(avg),
            );
            stat_card(
                ui,
                "Min Latency",
                &format!("{:.1} ms", if min_lat == f64::MAX { 0.0 } else { min_lat }),
                egui::Color32::from_rgb(150, 200, 255),
            );
            stat_card(
                ui,
                "Max Latency",
                &format!("{:.1} ms", max_lat),
                latency_color(max_lat),
            );

            // Overall connection quality verdict based on loss and latency thresholds
            let (verdict, color) = if s.total_sent == 0 {
                ("No data", egui::Color32::GRAY)
            } else if loss < 1.0 && avg < 50.0 {
                ("Excellent", egui::Color32::from_rgb(0, 255, 100))
            } else if loss < 5.0 && avg < 100.0 {
                ("Good", egui::Color32::from_rgb(150, 255, 50))
            } else if loss < 15.0 {
                ("Fair", egui::Color32::from_rgb(255, 200, 50))
            } else {
                ("Poor", egui::Color32::from_rgb(255, 80, 80))
            };
            stat_card(ui, "Connection", verdict, color);
        });

        ui.add_space(8.0);

        // --- Latency Over Time Chart ---
        // Plots all recorded latencies as a line chart using egui_plot.
        // X-axis is the sample index, Y-axis is latency in milliseconds.
        ui.heading("Latency Over Time");
        let latencies: Vec<[f64; 2]> = s
            .all_latencies
            .iter()
            .enumerate()
            .map(|(i, &lat)| [i as f64, lat])
            .collect();

        let line = Line::new(PlotPoints::new(latencies))
            .color(egui::Color32::from_rgb(100, 200, 255))
            .name("Latency (ms)");

        Plot::new("latency_plot")
            .height(180.0)
            .allow_drag(false)
            .allow_zoom(false)
            .show_axes(true)
            .y_axis_label("ms")
            .show(ui, |plot_ui| {
                plot_ui.line(line);
            });

        ui.add_space(8.0);

        // --- Interval Reports Table ---
        // Shows periodic summaries (e.g. every 60s) in a scrollable striped grid.
        // Most recent reports appear at the top (reverse iteration).
        if !s.interval_reports.is_empty() {
            ui.heading("Interval Reports");
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    egui::Grid::new("reports_grid")
                        .striped(true)
                        .min_col_width(60.0)
                        .show(ui, |ui| {
                            // Table header
                            ui.strong("Time");
                            ui.strong("Pings");
                            ui.strong("OK");
                            ui.strong("Fail");
                            ui.strong("Loss%");
                            ui.strong("Avg ms");
                            ui.strong("Min ms");
                            ui.strong("Max ms");
                            ui.end_row();

                            // Table rows — newest first
                            for report in s.interval_reports.iter().rev() {
                                ui.label(format!(
                                    "{}-{}",
                                    report.start_time.format("%H:%M:%S"),
                                    report.end_time.format("%H:%M:%S")
                                ));
                                ui.label(report.total_pings.to_string());
                                ui.label(report.successful.to_string());
                                ui.label(report.failed.to_string());
                                ui.colored_label(
                                    loss_color(report.packet_loss_pct),
                                    format!("{:.1}", report.packet_loss_pct),
                                );
                                ui.label(format!("{:.1}", report.avg_latency_ms));
                                ui.label(format!("{:.1}", report.min_latency_ms));
                                ui.label(format!("{:.1}", report.max_latency_ms));
                                ui.end_row();
                            }
                        });
                });
        }
    }

    /// Render the Console tab: a scrolling log of individual ping results.
    ///
    /// Log entries are cloned out of the lock into a local Vec to minimize
    /// lock hold time. Color-coded: green for success, red for timeouts.
    fn render_console(&mut self, ui: &mut egui::Ui) {
        // Toolbar: auto-scroll toggle and live/stopped indicator
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.auto_scroll, "Auto-scroll");

            let running = {
                let s = self.state.read().unwrap_or_else(|e| e.into_inner());
                s.running
            };
            if running {
                ui.colored_label(egui::Color32::from_rgb(100, 255, 100), "🟢 LIVE");
            } else {
                ui.colored_label(egui::Color32::from_rgb(255, 100, 100), "🔴 STOPPED");
            }
        });
        ui.separator();

        // Clone log entries out of the lock to avoid holding it during rendering
        let log_entries: Vec<(String, String)> = {
            let s = self.state.read().unwrap_or_else(|e| e.into_inner());
            s.log_entries
                .iter()
                .map(|e| {
                    let color_hint = e.message.clone();
                    (e.message.clone(), color_hint)
                })
                .collect()
        };

        // Scrollable log area with optional stick-to-bottom behavior
        let scroll = egui::ScrollArea::vertical().auto_shrink([false; 2]);
        let scroll = if self.auto_scroll {
            scroll.stick_to_bottom(true)
        } else {
            scroll
        };

        scroll.show(ui, |ui| {
            // Use monospace font for aligned log output
            ui.style_mut().override_font_id = Some(egui::FontId::monospace(12.0));
            for (msg, _) in &log_entries {
                // Red for failures, green for successful replies
                let color = if msg.contains("timed out") || msg.contains("unreachable") {
                    egui::Color32::from_rgb(255, 100, 100)
                } else {
                    egui::Color32::from_rgb(180, 220, 180)
                };
                ui.colored_label(color, msg);
            }
        });
    }

    /// Push the sidebar config fields into shared state and signal
    /// the pinger to reset its interval tracking.
    fn apply_config(&mut self) {
        let timeout = self.config_timeout.parse::<u32>().unwrap_or(2000);
        let interval = self.config_interval.parse::<u64>().unwrap_or(60);

        let mut s = self.state.write().unwrap_or_else(|e| e.into_inner());
        s.config = PingConfig {
            target: self.config_target.clone(),
            timeout_ms: timeout,
            interval_secs: interval,
        };
        // Signal the pinger thread to reset interval accumulation
        s.config_changed = true;
    }
}

/// Render a compact stat card with a label and a large colored value.
/// Used in the Monitor tab's stats row.
fn stat_card(ui: &mut egui::Ui, label: &str, value: &str, color: egui::Color32) {
    egui::Frame::new()
        .inner_margin(egui::Margin::same(8))
        .corner_radius(4.0)
        .fill(egui::Color32::from_rgb(40, 40, 50))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.small(label);
                ui.colored_label(color, egui::RichText::new(value).size(18.0).strong());
            });
        });
}

/// Map packet loss percentage to a traffic-light color:
/// green (<1%), yellow (1-5%), red (>5%)
fn loss_color(loss: f64) -> egui::Color32 {
    if loss < 1.0 {
        egui::Color32::from_rgb(100, 255, 100)
    } else if loss < 5.0 {
        egui::Color32::from_rgb(255, 255, 100)
    } else {
        egui::Color32::from_rgb(255, 80, 80)
    }
}

/// Map latency to a traffic-light color:
/// green (<30ms), yellow (30-100ms), red (>100ms)
fn latency_color(ms: f64) -> egui::Color32 {
    if ms < 30.0 {
        egui::Color32::from_rgb(100, 255, 100)
    } else if ms < 100.0 {
        egui::Color32::from_rgb(255, 255, 100)
    } else {
        egui::Color32::from_rgb(255, 80, 80)
    }
}
