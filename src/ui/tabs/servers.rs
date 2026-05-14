//! # Servers Tab - Quick-check game/service connectivity
//!
//! On-demand parallel connectivity test for all target presets.
//! Shows a grid with status icons, latency, and category grouping.

use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::core::config::TargetPreset;
use crate::core::preset_packs::SavedPresetPack;
use crate::core::server_check::{CheckStatus, ServerCheckResult};
use crate::ui::components::sidebar::SidebarState;

/// Persistent state for the Servers tab
pub struct ServersState {
    pub results: Vec<ServerCheckResult>,
    pub checking: bool,
    pending_results: Option<Arc<Mutex<Vec<Option<ServerCheckResult>>>>>,
    pub expected_count: usize,
    /// Which pack is selected for testing (by name)
    pub selected_server_pack: Option<String>,
    /// Local copy of presets to test (independent from sidebar)
    pub testing_presets: Vec<TargetPreset>,
}

impl ServersState {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
            checking: false,
            pending_results: None,
            expected_count: 0,
            selected_server_pack: None,
            testing_presets: Vec::new(),
        }
    }
}

/// Render the Servers tab
pub fn render(ui: &mut egui::Ui, all_packs: &[SavedPresetPack], servers: &mut ServersState, sidebar: &SidebarState) {
    // Poll for completed background check
    if servers.checking {
        let all_done = servers.pending_results.as_ref().map(|pending| {
            let guard = pending.lock().unwrap_or_else(|e| e.into_inner());
            let done_count = guard.iter().filter(|r| r.is_some()).count();
            if done_count >= servers.expected_count {
                servers.results = guard.iter().filter_map(|r| r.clone()).collect();
                true
            } else {
                false
            }
        }).unwrap_or(false);

        if all_done {
            servers.checking = false;
            servers.pending_results = None;
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.heading("🎮 Server Connectivity");
            ui.add_space(4.0);

            // Pack selector dropdown
            let current_label = servers.selected_server_pack.clone().unwrap_or_else(|| "Current presets".into());
            ui.horizontal(|ui| {
                ui.label("Pack:");
                egui::ComboBox::from_id_salt("server_pack_selector")
                    .selected_text(&current_label)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(servers.selected_server_pack.is_none(), "Current presets").clicked() {
                            servers.selected_server_pack = None;
                            servers.testing_presets = sidebar.presets.clone();
                        }
                        for pack in all_packs {
                            let selected = servers.selected_server_pack.as_deref() == Some(&pack.name);
                            if ui.selectable_label(selected, &pack.name).clicked() {
                                servers.selected_server_pack = Some(pack.name.clone());
                                servers.testing_presets = pack.presets.clone();
                            }
                        }
                    });
            });
            ui.add_space(4.0);

            // Use testing_presets if populated, otherwise fall back to sidebar presets
            let presets: Vec<TargetPreset> = if servers.testing_presets.is_empty() {
                sidebar.presets.clone()
            } else {
                servers.testing_presets.clone()
            };

            ui.horizontal(|ui| {
                let btn_text = if servers.checking { "⏳ Checking..." } else { "🔍 Check All" };
                if ui.add_enabled(!servers.checking, egui::Button::new(btn_text)).clicked() {
                    start_check_all(&presets, servers);
                }

                if !servers.results.is_empty() {
                    let ok_count = servers.results.iter().filter(|r| matches!(r.status, CheckStatus::Ok)).count();
                    let total = servers.results.len();
                    ui.label(format!("{}/{} reachable", ok_count, total));

                    if let Some(first) = servers.results.first() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!("Last check: {}", first.checked_at.format("%H:%M:%S")))
                                    .small()
                                    .weak(),
                            );
                        });
                    }
                }
            });

            ui.add_space(8.0);

            if servers.results.is_empty() && !servers.checking {
                ui.label("Press \"Check All\" to test connectivity to all presets.");
                return;
            }

            // Group results by category
            let categories: Vec<String> = {
                let mut cats: Vec<String> = servers.results.iter()
                    .map(|r| if r.category.is_empty() { "Other".to_string() } else { r.category.clone() })
                    .collect();
                cats.dedup();
                let mut seen = std::collections::HashSet::new();
                cats.retain(|c| seen.insert(c.clone()));
                cats
            };

            for cat in &categories {
                let filtered: Vec<_> = servers.results.iter()
                    .filter(|r| {
                        let rc = if r.category.is_empty() { "Other" } else { &r.category };
                        rc == cat
                    })
                    .collect();
                if !filtered.is_empty() {
                    render_results_table(ui, cat, &filtered);
                }
            }
        });
}

fn render_results_table(ui: &mut egui::Ui, title: &str, results: &[&ServerCheckResult]) {
    let header_id = ui.make_persistent_id(format!("servers_{}", title));
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), header_id, true)
        .show_header(ui, |ui| {
            let ok = results.iter().filter(|r| matches!(r.status, CheckStatus::Ok)).count();
            let icon = if ok == results.len() { "🟢" } else if ok > 0 { "🟡" } else { "🔴" };
            ui.label(format!("{} {} ({}/{})", icon, title, ok, results.len()));
        })
        .body(|ui| {
            egui::Grid::new(format!("servers_grid_{}", title))
                .striped(true)
                .min_col_width(40.0)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Status").strong());
                    ui.label(egui::RichText::new("Name").strong());
                    ui.label(egui::RichText::new("Host").strong());
                    ui.label(egui::RichText::new("Mode").strong());
                    ui.label(egui::RichText::new("Latency").strong());
                    ui.end_row();

                    for result in results {
                        let (icon, color) = match &result.status {
                            CheckStatus::Ok => ("🟢", egui::Color32::from_rgb(100, 200, 100)),
                            CheckStatus::Timeout => ("🔴", egui::Color32::from_rgb(200, 80, 80)),
                            CheckStatus::Error(_) => ("🟡", egui::Color32::from_rgb(200, 200, 80)),
                        };

                        ui.label(icon);
                        ui.label(&result.name);
                        ui.label(egui::RichText::new(&result.host).weak());
                        ui.label(egui::RichText::new(result.mode.label()).weak());

                        match &result.status {
                            CheckStatus::Ok => {
                                let ms = result.latency_ms.unwrap_or(0.0);
                                let lat_color = if ms < 50.0 {
                                    egui::Color32::from_rgb(100, 200, 100)
                                } else if ms < 100.0 {
                                    egui::Color32::from_rgb(200, 200, 80)
                                } else {
                                    egui::Color32::from_rgb(200, 130, 80)
                                };
                                ui.label(egui::RichText::new(format!("{:.0}ms", ms)).color(lat_color));
                            }
                            CheckStatus::Timeout => {
                                ui.label(egui::RichText::new("Timeout").color(color));
                            }
                            CheckStatus::Error(e) => {
                                ui.label(egui::RichText::new(e.as_str()).color(color));
                            }
                        }
                        ui.end_row();
                    }
                });
        });

    ui.add_space(4.0);
}

fn start_check_all(presets: &[TargetPreset], servers: &mut ServersState) {
    servers.checking = true;
    servers.expected_count = presets.len();

    let presets_clone: Vec<TargetPreset> = presets.to_vec();
    let results: Arc<Mutex<Vec<Option<ServerCheckResult>>>> =
        Arc::new(Mutex::new(vec![None; presets_clone.len()]));
    servers.pending_results = Some(results.clone());

    for (idx, preset) in presets_clone.into_iter().enumerate() {
        let results = results.clone();
        std::thread::spawn(move || {
            let result = crate::core::server_check::check_server(&preset, 3000);
            let mut guard = results.lock().unwrap_or_else(|e| e.into_inner());
            guard[idx] = Some(result);
        });
    }
}
