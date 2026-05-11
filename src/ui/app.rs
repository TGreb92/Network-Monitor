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
    /// Whether background threads have been spawned (deferred to first frame)
    initialized: bool,
}

impl NetworkMonitorApp {
    pub fn new(state: SharedState, _cc: &eframe::CreationContext<'_>) -> Self {
        let saved = config::load();

        {
            let mut shared = state.lock().unwrap_or_else(|err| err.into_inner());
            shared.config = PingConfig::from_saved(&saved);
            shared.gateway.enabled = saved.gateway_enabled;
        }

        let mut sidebar = SidebarState::new(saved.presets.clone(), saved.selected_preset, saved.export_path.clone());
        sidebar.auto_export_csv = saved.auto_export_csv;
        sidebar.auto_export_json = saved.auto_export_json;
        sidebar.auto_export_isp = saved.auto_export_isp;
        sidebar.auto_export_log = saved.auto_export_log;

        Self {
            state,
            active_tab: Tab::Monitor,
            sidebar,
            console: ConsoleState::new(),
            config_tab: ConfigState::from_saved(&saved),
            monitor: MonitorState::new(),
            initialized: false,
        }
    }

    /// Spawn background threads on the first frame, after the window is fully visible.
    fn initialize_once(&mut self) {
        if self.initialized {
            return;
        }
        self.initialized = true;

        // Spawn pinger threads
        pinger::start_pinger(self.state.clone());
        pinger::start_gateway_pinger(self.state.clone());

        // Detect gateway in background
        let saved = config::load();
        if saved.auto_detect_gateway {
            let state_clone = self.state.clone();
            std::thread::spawn(move || {
                if let Some(ip) = pinger::detect_gateway() {
                    let mut shared = state_clone.lock().unwrap_or_else(|err| err.into_inner());
                    shared.gateway.ip = Some(ip);
                }
            });
        }
    }
}

impl eframe::App for NetworkMonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Spawn background threads after the window is visible (first frame)
        self.initialize_once();

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
