//! # App - Main application struct and tab routing
//!
//! Owns all component state and delegates rendering to each module.

use eframe::egui;

use crate::core::config;
use crate::core::pinger;
use crate::core::state::{PingConfig, SharedState, lock_state};
use crate::ui::tabs::config::ConfigState;
use crate::ui::tabs::console::{self, ConsoleState};
use crate::ui::tabs::monitor::{self, MonitorState};
use crate::ui::components::sidebar::{self, SidebarState};
use crate::ui::tabs::help;

#[derive(PartialEq)]
enum Tab {
    Monitor,
    Console,
    Config,
    Help,
}

/// Main application struct - owns shared state and all component state
pub struct NetworkMonitorApp {
    state: SharedState,
    active_tab: Tab,
    sidebar: SidebarState,
    console: ConsoleState,
    config_tab: ConfigState,
    monitor: MonitorState,
    /// Frame counter for startup sequence; None after startup completes
    startup_frames_remaining: Option<u32>,
}

impl NetworkMonitorApp {
    pub fn new(state: SharedState, _cc: &eframe::CreationContext<'_>) -> Self {
        let saved = config::load();

        {
            let mut shared = lock_state(&state);
            shared.config = PingConfig::from_saved(&saved);
            shared.gateway.enabled = saved.gateway_enabled;
        }

        let mut sidebar = SidebarState::new(saved.presets.clone(), saved.selected_preset, saved.export_path.clone());
        sidebar.exports = crate::ui::components::export_import::ExportState::from_saved(&saved);
        sidebar.notifications = crate::ui::components::notifications::NotificationState::from_saved(&saved);

        Self {
            state,
            active_tab: Tab::Monitor,
            sidebar,
            console: ConsoleState::new(),
            config_tab: ConfigState::from_saved(&saved),
            monitor: MonitorState::new(),
            startup_frames_remaining: Some(5),
        }
    }

    /// Spawn background threads after the window has rendered several frames.
    /// Also forces window focus on the first few frames to combat Windows
    /// stealing focus during OpenGL/glow context setup.
    fn startup_sequence(&mut self, ctx: &egui::Context) {
        let Some(remaining) = self.startup_frames_remaining else { return };

        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);

        // Spawn threads on frame 3 (window is fully painted by then)
        if remaining == 3 {
            pinger::start_pinger(self.state.clone());
            pinger::start_gateway_pinger(self.state.clone());

            let saved = config::load();
            if saved.auto_detect_gateway {
                let state_clone = self.state.clone();
                std::thread::spawn(move || {
                    if let Some(ip) = pinger::detect_gateway() {
                        let mut shared = lock_state(&state_clone);
                        shared.gateway.ip = Some(ip);
                    }
                });
            }
        }

        self.startup_frames_remaining = if remaining <= 1 { None } else { Some(remaining - 1) };
    }
}

impl eframe::App for NetworkMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.startup_sequence(ctx);

        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        sidebar::render(ctx, &self.state, &mut self.sidebar);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Monitor, "📊 Monitor");
                ui.selectable_value(&mut self.active_tab, Tab::Console, "🖥 Console");
                ui.selectable_value(&mut self.active_tab, Tab::Config, "⚙ Config");
                ui.selectable_value(&mut self.active_tab, Tab::Help, "❓ Help");
            });
            ui.separator();

            match self.active_tab {
                Tab::Monitor => {
                    let shared = lock_state(&self.state);
                    monitor::render(ui, &shared, &mut self.monitor);
                }
                Tab::Console => console::render(ui, &self.state, &mut self.console),
                Tab::Config => crate::ui::tabs::config::render(ui, &self.state, &mut self.config_tab, &mut self.sidebar),
                Tab::Help => help::render(ui),
            }
        });
    }
}
