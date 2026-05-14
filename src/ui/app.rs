//! # App - Main application struct and tab routing
//!
//! Owns all component state and delegates rendering to each module.

use eframe::egui;

use crate::core::config;
use crate::core::pinger;
use crate::core::preset_packs;
use crate::core::state::{PingConfig, SharedState, ShutdownSignal, lock_state};
use crate::ui::tabs::config::ConfigState;
use crate::ui::tabs::console::{self, ConsoleState};
use crate::ui::tabs::monitor::{self, MonitorState};
use crate::ui::tabs::presets::{self, PresetsTabState};
use crate::ui::tabs::servers::{self, ServersState};
use crate::ui::components::sidebar::{self, SidebarState};
use crate::ui::components::tray::{TrayState, TrayAction};
use crate::ui::tabs::help;
#[cfg(debug_assertions)]
use crate::ui::tabs::debug;

#[derive(PartialEq)]
enum Tab {
    Monitor,
    Console,
    Presets,
    Servers,
    Config,
    Help,
    #[cfg(debug_assertions)]
    Debug,
}

/// Main application struct - owns shared state and all component state
pub struct NetworkMonitorApp {
    state: SharedState,
    active_tab: Tab,
    sidebar: SidebarState,
    console: ConsoleState,
    config_tab: ConfigState,
    monitor: MonitorState,
    presets_tab: PresetsTabState,
    servers: ServersState,
    tray: TrayState,
    /// Shutdown signals for background threads
    pinger_shutdown: Option<ShutdownSignal>,
    gateway_shutdown: Option<ShutdownSignal>,
    modem_health_shutdown: Option<ShutdownSignal>,
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
            shared.modem_health_enabled = saved.modem_health_enabled;
            shared.modem_health_url = saved.modem_health_url.clone();
            shared.modem_health_interval_secs = saved.modem_health_interval_secs;
            shared.modem_struggle_window_mins = saved.modem_struggle_window_mins;
            if saved.modem_health_enabled {
                shared.modem_http_status = crate::core::state::ModemHttpStatus::Unknown;
            }
        }

        let mut sidebar = SidebarState::new(saved.presets.clone(), saved.selected_preset, saved.export_path.clone());
        sidebar.exports = crate::ui::components::export_import::ExportState::from_saved(&saved);
        sidebar.notifications = crate::ui::components::notifications::NotificationState::from_saved(&saved);

        let packs_config = preset_packs::load();
        let presets_tab = PresetsTabState::from_packs_config(&packs_config);

        Self {
            state,
            active_tab: Tab::Monitor,
            sidebar,
            console: ConsoleState::new(),
            config_tab: ConfigState::from_saved(&saved),
            monitor: MonitorState::new(),
            presets_tab,
            servers: ServersState::new(),
            tray: TrayState::new(),
            pinger_shutdown: None,
            gateway_shutdown: None,
            modem_health_shutdown: None,
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
            let (_, pinger_sig) = pinger::start_pinger(self.state.clone());
            let (_, gw_sig) = pinger::start_gateway_pinger(self.state.clone());
            let (_, modem_sig) = crate::core::modem_health::start_modem_health_checker(self.state.clone());
            self.pinger_shutdown = Some(pinger_sig);
            self.gateway_shutdown = Some(gw_sig);
            self.modem_health_shutdown = Some(modem_sig);

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

    /// Signal all background threads to shut down
    fn shutdown_all_threads(&self) {
        for signal in [&self.pinger_shutdown, &self.gateway_shutdown, &self.modem_health_shutdown] {
            if let Some(s) = signal {
                s.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
}

impl eframe::App for NetworkMonitorApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shutdown_all_threads();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.startup_sequence(ctx);

        // Always repaint periodically (even when minimized to tray)
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        // Process tray menu actions (must run even when minimized)
        let muted = self.sidebar.notifications.muted;
        match self.tray.update(&self.state, ctx, muted) {
            TrayAction::ShowWindow => {
                // Window was already shown by the tray event handler via Win32.
                // Just clear the flag so UI rendering resumes.
                self.tray.minimized_to_tray = false;
            }
            TrayAction::ToggleTest => {
                let mut shared = lock_state(&self.state);
                if shared.running {
                    shared.stop();
                } else {
                    shared.start();
                }
            }
            TrayAction::ToggleMute => {
                self.sidebar.notifications.muted = !self.sidebar.notifications.muted;
            }
            TrayAction::MinimizeToTray => {
                self.tray.minimized_to_tray = true;
                crate::ui::components::tray::hide_window(ctx);
            }
            TrayAction::Exit => {
                self.shutdown_all_threads();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            TrayAction::None => {}
        }

        // Skip UI rendering if minimized to tray (but keep processing above)
        if self.tray.minimized_to_tray {
            // Still sync notifications so toasts fire while minimized
            crate::ui::components::notifications::sync_and_fire(
                ctx, &self.state, &mut self.sidebar.notifications,
            );
            crate::ui::components::export_import::check_auto_export_pending(
                &self.state, &mut self.sidebar.exports,
            );
            return;
        }

        sidebar::render(ctx, &self.state, &mut self.sidebar);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Monitor, "📊 Monitor");
                ui.selectable_value(&mut self.active_tab, Tab::Console, "🖥 Console");
                ui.selectable_value(&mut self.active_tab, Tab::Presets, "📋 Presets");
                ui.selectable_value(&mut self.active_tab, Tab::Servers, "🎮 Servers");
                ui.selectable_value(&mut self.active_tab, Tab::Config, "⚙ Config");
                ui.selectable_value(&mut self.active_tab, Tab::Help, "❓ Help");
                #[cfg(debug_assertions)]
                ui.selectable_value(&mut self.active_tab, Tab::Debug, "🔧 Debug");
            });
            ui.separator();

            match self.active_tab {
                Tab::Monitor => {
                    let shared = lock_state(&self.state);
                    monitor::render(ui, &shared, &mut self.monitor);
                }
                Tab::Console => console::render(ui, &self.state, &mut self.console),
                Tab::Presets => presets::render(ui, &mut self.presets_tab, &mut self.sidebar),
                Tab::Servers => {
                    let all_packs = self.presets_tab.all_packs();
                    servers::render(ui, &all_packs, &mut self.servers, &self.sidebar);
                }
                Tab::Config => crate::ui::tabs::config::render(ui, &self.state, &mut self.config_tab, &mut self.sidebar),
                Tab::Help => help::render(ui),
                #[cfg(debug_assertions)]
                Tab::Debug => debug::render(ui, &self.state),
            }
        });
    }
}
