# Network Monitor

A lightweight Rust desktop application that monitors network connectivity by pinging a target host. Built with [egui](https://github.com/emilk/egui) for a native, dark-themed GUI.

## Download

Grab the latest `network-monitor.exe` from the [Releases](../../releases) page — no install required.

Or build from source (see [Building](#building) below).

## Features

### 📊 Monitoring
- **Real-time ping monitoring** with configurable target, timeout, and frequency
- **Live statistics** — packet loss %, avg/min/max latency, jitter, connection verdict
- **Latency chart** — time-based graph with selectable time window (1m/5m/15m/30m/All), tiered ping thresholds, and loss period markers
- **Interval reports** — periodic summaries with loss event counts, filtered by selected time window
- **Loss batch tracking** — counts distinct connectivity drops, not just individual timeouts
- **Tiered ping detection** — Elevated/High/Critical thresholds with batch tracking
- **Configurable test duration** — set a time limit or run indefinitely
- **Time-windowed stats** — all stat cards follow the selected chart time window

### 🌐 Gateway & Modem Monitoring
- **Auto-detect gateway** — finds your router IP via `ipconfig`
- **Parallel gateway pings** — monitors both your router and external target simultaneously
- **ISP vs local diagnosis** — determines if packet loss is on your LAN or your ISP's network
- **Modem HTTP health check** — periodic HTTP GET to your modem's status page to detect CPU/firmware issues
- **Modem struggle detection** — tracks external loss batches while gateway is healthy; detects dying modem CPU
- **Configurable detection window** — adjust the struggle detection window (2–30 minutes) to match your issue pattern
- **6-level diagnosis** — Local network → Modem CPU struggling → Modem may be struggling → Modem web UI unreachable → ISP/route issue → All clear

### 📋 Presets & Configuration
- **Named target presets** — save frequently used targets (Google DNS, Cloudflare, etc.)
- **Quick-switch dropdown** — change targets without retyping
- **Add/edit/delete presets** in the Config tab
- **TOML persistence** — all settings saved to `network-monitor.toml` next to the executable

### 🔔 Notifications
- **Toast notifications** on loss events, gateway loss, and tiered high pings
- **Severity-based cooldown** — higher severity breaks through cooldown timer
- **Mute toggle** — quickly silence all notifications from the sidebar or tray

### 📥 Export & Import
- **CSV export** — ping results and interval reports
- **JSON export** — full session data including gateway and modem health analysis
- **ISP Report** — human-readable text report with gateway diagnosis, modem health analysis, loss timeline, and interval breakdown
- **Console Log** — raw ping log dump
- **JSON import** — load a previous export to review stats, chart, and logs in the UI
- **Auto-export on stop** — configure which formats to export automatically when a test ends
- **Configurable export folder** — native folder picker or default `exports/` subfolder

### 🖥 Console
- Live scrolling log: `[HH:MM:SS] #42 Reply from 8.8.8.8: time=15ms`
- Auto-scroll toggle with LIVE/STOPPED indicator

### 🗔 System Tray
- **Minimize to tray** — hides from taskbar, keeps monitoring in background
- **Tray menu** — Show/Stop/Start/Mute/Minimize/Exit
- **Double-click to restore** window from tray

## Architecture

```
src/
├── main.rs                  — Entry point, COM init, #![windows_subsystem = "windows"]
├── core/
│   ├── config.rs            — TOML config persistence, presets
│   ├── export.rs            — CSV, JSON, ISP report, console log writers
│   ├── import.rs            — JSON import to reconstruct session
│   ├── modem_health.rs      — HTTP health check background thread
│   ├── pinger.rs            — Background ping thread, gateway detection
│   └── models/
│       ├── state.rs         — Shared state (PingState), shutdown signals, diagnosis
│       ├── types.rs         — PingResult, PingConfig, IntervalReport
│       ├── trackers.rs      — LossBatchTracker, JitterTracker, GatewayStats
│       ├── tiers.rs         — PingTier enum, TieredPingTracker, thresholds
│       └── json_types.rs    — Serde types for JSON export/import
└── ui/
    ├── app.rs               — Main App struct, tab routing, thread lifecycle
    ├── components/
    │   ├── helpers.rs        — Formatting utilities (stat cards, color helpers)
    │   ├── notifications.rs  — Toast notification logic, cooldown, severity
    │   ├── presets.rs        — Preset CRUD UI
    │   ├── export_import.rs  — Export buttons, auto-export, JSON import
    │   ├── sidebar.rs        — Controls panel, target selector, stats
    │   └── tray.rs           — System tray icon, menu, hide/show via Win32
    └── tabs/
        ├── config.rs         — Config tab: ping, gateway, modem health, notifications
        ├── console.rs        — Console tab: live ping log viewer
        ├── debug.rs          — Debug tab (debug builds only): test toasts, thread status
        ├── help.rs           — Help tab: metric explanations and usage guide
        ├── monitor.rs        — Monitor tab: windowed stats, chart, reports, diagnosis
```

### Design Decisions

- **Arc\<Mutex\>** — simple shared state between threads (pinger + gateway + modem + GUI)
- **Bounded collections** — all `VecDeque` collections capped (7200 results, 2000 log entries) to prevent unbounded memory growth
- **ShutdownSignal** — per-thread `Arc<AtomicBool>` for clean shutdown on window close
- **Thread heartbeats** — each background thread writes `Instant::now()` for liveness monitoring (debug tab)
- **SidebarSnapshot / ConfigSnapshot** — reads all needed data in one mutex lock, releases lock, then renders
- **Deferred initialization** — background threads spawn after several rendered frames, not during window creation
- **CREATE_NO_WINDOW** — `0x08000000` flag on all Windows subprocess spawns to prevent console popups
- **Poisoned lock recovery** — `.unwrap_or_else(|e| e.into_inner())` on all mutex locks
- **COM initialization** — `CoInitializeEx(COINIT_APARTMENTTHREADED)` on main thread before eframe for notify-rust/rfd compatibility

## Installation

### Download

Download `network-monitor.exe` from the [Releases](../../releases) page and run it — no installation needed.

### Building

**Prerequisites:** Rust toolchain (1.85+), Windows

```bash
git clone https://github.com/TGreb92/Network-Monitor.git
cd Network-Monitor
cargo build --release
```

The binary will be at `target/release/network-monitor.exe`.

The release profile is optimized for size:
- `opt-level = "s"` — optimize for binary size
- `lto = true` — link-time optimization
- `strip = true` — strip debug symbols
- `codegen-units = 1` — single codegen unit

Debug builds include a console window and a Debug tab with test toast buttons, thread heartbeat monitoring, and event simulation.

## Usage

1. Launch `network-monitor.exe`
2. Select a target from the dropdown (or configure your own in the Config tab)
3. Click **▶ Start** to begin monitoring
4. Use the tabs to switch views:

| Tab | Purpose |
|-----|---------|
| **📊 Monitor** | Latency chart, windowed stats, interval reports, network diagnosis |
| **🖥 Console** | Live scrolling ping log |
| **⚙ Config** | Ping settings, presets, gateway, modem health, notifications, export |
| **❓ Help** | Explanation of all metrics and UI elements |
| **🔧 Debug** | Thread status, test toasts, event simulation *(debug builds only)* |

### Modem Health Detection

Enable **Modem HTTP Health Check** in the Config tab to periodically test your modem's web interface. The app detects modem CPU issues by correlating:
- External packet loss while your gateway (router) shows no symptoms
- Modem web UI becoming unreachable

Adjust the **struggle detection window** (default 5 min) to match your disconnection pattern (typically 2–10 minutes for a dying modem).

### Exporting

Use the sidebar export buttons (CSV, JSON, ISP Report, Console Log) or enable **auto-export on stop** in the Config tab. The ISP report and JSON export include full modem health analysis and diagnosis.

### Importing

Click **📥 Load JSON…** in the sidebar to import a previous JSON export and review the session data in the UI.

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| eframe | 0.31 | Native GUI framework (glow backend) |
| egui_plot | 0.31 | Latency chart plotting |
| chrono | 0.4 | Local time formatting |
| csv | 1 | CSV export |
| serde | 1 | Serialization framework |
| serde_json | 1 | JSON export/import |
| toml | 0.8 | Config file persistence |
| rfd | 0.15 | Native file/folder picker dialogs |
| notify-rust | 4 | Windows toast notifications |
| tray-icon | 0.19 | System tray icon and menu |
| windows-sys | 0.59 | Win32 API (COM init, ShowWindow) |
| image | 0.25 | Tray icon image loading |

## License

MIT
