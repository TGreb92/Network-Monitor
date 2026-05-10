# Network Monitor

A lightweight Rust desktop application that monitors network connectivity by pinging a target host. Built with [egui](https://github.com/emilk/egui) for a native, cross-platform GUI.

## Features

- **Real-time ping monitoring** — Continuously pings a configurable target (default: `8.8.8.8`)
- **Live statistics dashboard** — Packet loss %, average/min/max latency, connection verdict
- **Latency chart** — Visual latency-over-time graph via `egui_plot`
- **Interval reports** — Periodic summaries with configurable reporting intervals
- **Console log** — Scrollable ping log with color-coded success/failure entries
- **Configurable** — Change target host, timeout, and report interval at runtime
- **Low footprint** — Minimal CPU/memory usage, runs quietly in the background
- **Windows-optimized** — Hides console window, uses `CREATE_NO_WINDOW` flag for subprocess spawning

## Architecture

```
src/
├── main.rs    — Entry point, hides console, launches eframe
├── state.rs   — Shared state types with bounded VecDeque collections
├── pinger.rs  — Background ping thread with Windows CREATE_NO_WINDOW flag
└── app.rs     — egui GUI with Monitor and Console tabs
```

### Design Decisions

- **RwLock over Mutex** — GUI reads far more frequently than the pinger writes
- **Bounded collections** — All `VecDeque` collections have caps (7200 results, 2000 log entries) to prevent unbounded memory growth
- **Poisoned lock recovery** — Uses `.unwrap_or_else(|e| e.into_inner())` to survive mutex poisoning
- **Local time** — All timestamps use `chrono::Local` for human-readable output
- **CREATE_NO_WINDOW** — Prevents console popups when spawning `ping.exe` on Windows

## Installation

### Prerequisites

- Rust toolchain (1.85+)
- Windows (primary target; uses Windows-specific `ping` flags)

### Build

```bash
cargo build --release
```

The optimized binary will be at `target/release/network-monitor.exe`.

### Release Profile

The release build is optimized for size:
- `opt-level = "s"` — Optimize for binary size
- `lto = true` — Link-time optimization
- `strip = true` — Strip debug symbols
- `codegen-units = 1` — Single codegen unit for better optimization

## Usage

1. Launch `network-monitor.exe`
2. Click **▶ Start** in the sidebar to begin pinging
3. Switch between **Monitor** and **Console** tabs
4. Adjust target, timeout, and interval in the sidebar, then click **Apply Config**

### Monitor Tab

- Live stat cards: packet loss, avg/min/max latency, connection quality verdict
- Latency chart showing trends over time
- Interval reports table with periodic summaries

### Console Tab

- Live scrolling log: `[HH:MM:SS] #seq Reply from host: time=Xms`
- Auto-scroll toggle
- Color-coded: green for success, red for timeouts
- LIVE/STOPPED indicator

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| eframe | 0.31 | Native GUI framework |
| egui_plot | 0.31 | Plotting library for latency charts |
| image | 0.25 | Image loading (PNG) |
| tray-icon | 0.19 | System tray support |
| chrono | 0.4 | Local time formatting |
| winapi | 0.3 | Windows API (FreeConsole) |

## Future Improvements

### 📈 Chart & UX Improvements
- [ ] **Time range selector** — Zoom into last 1m / 5m / 15m / 30m / custom range on the latency graph
- [ ] **Draggable time window** — Click and drag to select a section of the chart and view stats for just that period
- [ ] **Hover tooltips** — Show exact latency, timestamp, and sequence number when hovering over graph points
- [ ] **Min/Max/Avg overlay lines** — Toggle horizontal reference lines on the chart for quick visual comparison
- [ ] **Latency distribution histogram** — Separate view showing how latency values are distributed
- [ ] **Jitter graph** — Plot latency variance (difference between consecutive pings) alongside the latency graph
- [ ] **Split-view comparison** — View two time ranges side by side (e.g., "morning vs. evening")
- [ ] **Zoom & pan** — Mouse wheel zoom + click-drag pan on the latency chart
- [ ] **Interval sparklines** — Mini inline charts in the interval reports table showing latency trend per interval

### 🌐 Local Network & Router Testing
- [ ] **Auto-detect gateway** — Automatically find the default gateway IP and offer a one-click "Test Router" button
- [ ] **LAN health check** — Ping the router/gateway alongside the external target to distinguish local vs. ISP issues
- [ ] **Dual-graph view** — Show router latency and external latency on the same chart to visualize where delays originate
- [ ] **Hop-by-hop analysis** — Built-in traceroute with per-hop latency tracking over time
- [ ] **Network interface selector** — Choose which adapter to test through (useful for multi-NIC setups)

### 📋 Saved Targets & Presets
- [ ] **Named target list** — Save frequently used targets with custom names (e.g., "Google DNS", "Cloudflare", "Game Server EU")
- [ ] **Built-in presets** — Ship with common targets pre-configured (8.8.8.8, 1.1.1.1, 208.67.222.222, 9.9.9.9, auto-detected gateway)
- [ ] **Quick-switch dropdown** — Select a saved target from a dropdown in the sidebar without retyping
- [ ] **Add/edit/delete targets** — Manage your target list with a simple editor; persist to a local config file
- [ ] **Per-target history** — View past test results grouped by target name
- [ ] **Target groups** — Create groups (e.g., "DNS Servers", "Game Servers") and run batch tests

### 🔧 General
- [ ] Multiple simultaneous targets (side-by-side comparison)
- [ ] Export results to CSV/JSON
- [ ] System tray icon with minimize-to-tray
- [ ] Sound/desktop notifications on connection loss
- [ ] Configurable ping frequency (currently fixed at 1/sec)
- [ ] Dark/light theme toggle
- [ ] Historical data persistence across sessions
- [ ] DNS resolution monitoring
- [ ] Jitter calculation and display
- [ ] Customizable alert thresholds
- [ ] Cross-platform support (Linux/macOS ping command)

## License

MIT
