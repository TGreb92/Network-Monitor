# Network Monitor

A lightweight Rust desktop application that monitors network connectivity by pinging a target host. Built with [egui](https://github.com/emilk/egui) for a native, dark-themed GUI.

## Download

Grab the latest `network-monitor.exe` from the [Releases](../../releases) page — no install required.

Or build from source (see [Building](#building) below).

## Features

### 📊 Monitoring
- **Real-time ping monitoring** with configurable target, timeout, and frequency
- **Live statistics** — packet loss %, avg/min/max latency, jitter, connection verdict
- **Latency chart** — time-based graph with snap-to-nearest tooltips and loss period markers
- **Interval reports** — periodic summaries with loss event counts
- **Loss batch tracking** — counts distinct connectivity drops, not just individual timeouts
- **Configurable test duration** — set a time limit or run indefinitely

### 🌐 Gateway Monitoring
- **Auto-detect gateway** — finds your router IP via `ipconfig`
- **Parallel gateway pings** — monitors both your router and external target simultaneously
- **ISP vs local diagnosis** — determines if packet loss is on your LAN or your ISP's network

### 📋 Presets & Configuration
- **Named target presets** — save frequently used targets (Google DNS, Cloudflare, etc.)
- **Quick-switch dropdown** — change targets without retyping
- **Add/edit/delete presets** in the Config tab
- **TOML persistence** — all settings saved to `network-monitor.toml` next to the executable

### 📥 Export & Import
- **CSV export** — ping results and interval reports via the `csv` crate
- **JSON export** — full session data via `serde_json`, importable back into the app
- **ISP Report** — human-readable text report for support tickets, with gateway diagnosis and loss timeline
- **Console Log** — raw ping log dump
- **JSON import** — load a previous export to review stats, chart, and logs in the UI
- **Auto-export on stop** — configure which formats to export automatically when a test ends
- **Configurable export folder** — native folder picker or default `exports/` subfolder

### 🖥 Console
- Live scrolling log: `[HH:MM:SS] #42 Reply from 8.8.8.8: time=15ms`
- Auto-scroll toggle with LIVE/STOPPED indicator

## Architecture

```
src/
├── main.rs               — Entry point, #![windows_subsystem = "windows"]
├── core/
│   ├── config.rs         — TOML config persistence, presets
│   ├── export.rs         — CSV, JSON, ISP report, console log, JSON import
│   ├── pinger.rs         — Background ping threads, gateway detection
│   └── state.rs          — Shared state (PingState, JitterTracker, GatewayStats, etc.)
└── ui/
    ├── app.rs            — Main App struct, tab routing, deferred initialization
    ├── components/
    │   ├── helpers.rs    — Formatting utilities (stat cards, color helpers)
    │   ├── presets.rs    — Preset CRUD UI (add/edit/delete)
    │   └── sidebar.rs   — Controls panel, stats, export/import buttons
    └── tabs/
        ├── config.rs     — Config tab: ping settings, export options, preset editor
        ├── console.rs    — Console tab: live ping log viewer
        ├── help.rs       — Help tab: metric explanations and usage guide
        └── monitor.rs    — Monitor tab: latency chart, interval reports table
```

### Design Decisions

- **Arc\<Mutex\>** — simple shared state between 2 threads (pinger + GUI), minimal contention
- **Bounded collections** — all `VecDeque` collections capped (7200 results, 2000 log entries) to prevent unbounded memory growth
- **SidebarSnapshot** — reads all sidebar data in one mutex lock, releases lock, then renders
- **ConfigSnapshot** — pinger reads config once per loop, never holds the lock during subprocess execution
- **Deferred initialization** — background threads spawn on the first rendered frame, not during window creation
- **CREATE_NO_WINDOW** — `0x08000000` flag on all Windows subprocess spawns to prevent console popups
- **Poisoned lock recovery** — `.unwrap_or_else(|e| e.into_inner())` on all mutex locks

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

## Usage

1. Launch `network-monitor.exe`
2. Select a target from the dropdown (or configure your own in the Config tab)
3. Click **▶ Start** to begin monitoring
4. Use the tabs to switch views:

| Tab | Purpose |
|-----|---------|
| **📊 Monitor** | Latency chart, interval reports table, connection verdict |
| **🖥 Console** | Live scrolling ping log |
| **⚙ Config** | Ping settings, presets, export options, gateway config |
| **❓ Help** | Explanation of all metrics and UI elements |

### Exporting

Use the sidebar export buttons (CSV, JSON, ISP Report, Console Log) or enable **auto-export on stop** in the Config tab to automatically save results when a test ends.

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

## Roadmap

### ✅ Completed

- [x] Real-time ping monitoring with live stats
- [x] Latency chart with time-based X-axis, loss period markers, snap-to-nearest tooltips
- [x] Interval reports with loss event counts
- [x] Loss batch tracking (distinct connectivity drops)
- [x] Jitter calculation and display
- [x] Gateway auto-detection and parallel monitoring
- [x] ISP vs local network diagnosis
- [x] Named target presets with CRUD editor
- [x] Built-in presets (Google DNS, Cloudflare, Quad9, OpenDNS)
- [x] Quick-switch dropdown in sidebar
- [x] Configurable ping frequency, timeout, report interval
- [x] Configurable test duration with progress bar
- [x] Export: CSV, JSON, ISP report, console log
- [x] JSON import to review past sessions in the UI
- [x] Auto-export on stop (per-format toggles)
- [x] Configurable export folder with native folder picker
- [x] TOML config persistence
- [x] Console log with auto-scroll toggle
- [x] Help tab with full metric explanations

### 🔜 Planned

#### Chart & Visualization
- [ ] Chart time range selector (zoom to last 1m/5m/15m/30m/all)
- [ ] Latency distribution histogram
- [ ] Min/Max/Avg overlay reference lines on chart

#### Network & Diagnostics
- [ ] Multiple simultaneous targets (side-by-side comparison)
- [ ] Traceroute integration (hop-by-hop analysis)
- [ ] DNS resolution monitoring (track lookup time separately)
- [ ] Network interface selector (multi-NIC setups)
- [ ] Packet size configuration (test with different payload sizes)

#### Notifications & Alerts
- [ ] Sound/desktop notifications on connection loss
- [ ] Configurable alert thresholds (latency/loss triggers)

#### Session Management
- [ ] Historical session comparison (compare past runs)
- [ ] Session auto-save (local history for later review)

#### UX & Window Management
- [ ] System tray icon with minimize-to-tray
- [ ] Dark/light theme toggle
- [ ] Window always-on-top toggle
- [ ] Compact/mini mode (small floating window with loss % and latency)
- [ ] Startup auto-run (auto-start pinging on launch)
- [ ] Connection uptime tracker (total uptime % since app started)

#### Platform
- [ ] Cross-platform support (Linux/macOS ping commands)

## License

MIT
