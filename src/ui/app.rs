//! # App — Main application struct and tab routing
//!
//! Owns all component state and delegates rendering to each module.

use eframe::egui;

use crate::core::config;
use crate::core::pinger;
use crate::core::state::{PingConfig, SharedState};
use crate::ui::config_tab::{self, ConfigState};
use crate::ui::console::{self, ConsoleState};
use crate::ui::monitor::{self, MonitorState};
use crate::ui::sidebar::{self, SidebarState};
use crate::ui::help;

#[derive(PartialEq)]
enum Tab {
    Monitor,
    Console,
    Config,
    Help,
}

/// Main application struct — owns shared state and all component state
pub struct NetworkMonitorApp {
    state: SharedState,
    active_tab: Tab,
    sidebar: SidebarState,
    console: ConsoleState,
    config_tab: ConfigState,
    monitor: MonitorState,
}

impl NetworkMonitorApp {
    pub fn new(state: SharedState, _cc: &eframe::CreationContext<'_>) -> Self {
        let saved = config::load();

        {
            let mut shared = state.lock().unwrap_or_else(|err| err.into_inner());
            shared.config = PingConfig::from_saved(&saved);
            shared.gateway.enabled = saved.gateway_enabled;
        }

        // Detect gateway in a background thread to avoid stealing window focus
        if saved.auto_detect_gateway {
            let state_clone = state.clone();
            std::thread::spawn(move || {
                if let Some(ip) = pinger::detect_gateway() {
                    let mut shared = state_clone.lock().unwrap_or_else(|err| err.into_inner());
                    shared.gateway.ip = Some(ip);
                }
            });
        }

        Self {
            state,
            active_tab: Tab::Monitor,
            sidebar: SidebarState::new(saved.presets.clone(), saved.selected_preset),
            console: ConsoleState::new(),
            config_tab: ConfigState::from_saved(&saved),
            monitor: MonitorState::new(),
        }
    }
}

impl eframe::App for NetworkMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                    let shared = self.state.lock().unwrap_or_else(|err| err.into_inner());
                    monitor::render(ui, &shared, &mut self.monitor);
                }
                Tab::Console => console::render(ui, &self.state, &mut self.console),
                Tab::Config => config_tab::render(ui, &self.state, &mut self.config_tab, &mut self.sidebar),
                Tab::Help => help::render(ui),
            }
        });
    }
}
