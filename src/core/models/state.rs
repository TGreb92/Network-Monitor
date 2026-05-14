//! # Shared State
//!
//! Central `PingState` struct shared between the pinger thread and the GUI.
//! Uses `Arc<Mutex>` for thread-safe access. Individual data types and
//! trackers live in the `models` module.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub use super::types::*;
pub use super::trackers::*;
pub use super::tiers::*;

/// Tracks the current reporting interval and accumulates results
#[derive(Clone)]
pub struct IntervalTracker {
    pub start: Option<Instant>,
    pub start_time: Option<chrono::NaiveDateTime>,
    pub results: Vec<PingResult>,
}

impl IntervalTracker {
    pub fn new() -> Self {
        Self {
            start: None,
            start_time: None,
            results: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.start = None;
        self.start_time = None;
        self.results.clear();
    }
}

/// Modem HTTP health check status
#[derive(Clone, Debug, PartialEq)]
pub enum ModemHttpStatus {
    Disabled,
    Unknown,
    Ok,
    Failed(String),
}

/// Modem health-check and struggle-detection state
#[derive(Clone)]
pub struct ModemState {
    pub enabled: bool,
    pub url: String,
    pub interval_secs: u32,
    pub struggle_window_mins: u32,
    pub http_status: ModemHttpStatus,
    pub struggle_events: VecDeque<Instant>,
}

impl ModemState {
    pub fn new() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            interval_secs: 15,
            struggle_window_mins: 5,
            http_status: ModemHttpStatus::Disabled,
            struggle_events: VecDeque::with_capacity(32),
        }
    }

    /// Count modem struggle events in the last N minutes
    pub fn struggle_count(&self, minutes: u32) -> usize {
        let cutoff = Instant::now() - std::time::Duration::from_secs(minutes as u64 * 60);
        self.struggle_events.iter().filter(|&&t| t > cutoff).count()
    }

    /// Reset runtime data (keeps config)
    pub fn reset_runtime(&mut self) {
        self.struggle_events.clear();
    }
}

/// Thread alive timestamps - set by each background thread on every iteration
#[derive(Clone)]
pub struct ThreadHeartbeats {
    pub pinger: Option<Instant>,
    pub gateway: Option<Instant>,
    pub modem: Option<Instant>,
}

impl ThreadHeartbeats {
    pub fn new() -> Self {
        Self { pinger: None, gateway: None, modem: None }
    }
}

/// Central shared state accessed by both the pinger thread (writer) and the GUI (reader).
#[derive(Clone)]
pub struct PingState {
    pub config: PingConfig,
    pub running: bool,
    pub results: VecDeque<PingResult>,
    pub log_entries: VecDeque<PingLogEntry>,
    pub interval_reports: VecDeque<IntervalReport>,
    pub all_latencies: VecDeque<f64>,
    pub total_sent: u64,
    pub total_received: u64,
    pub seq_counter: u64,
    pub config_changed: bool,
    pub start_time: Option<Instant>,
    pub jitter: JitterTracker,
    pub gateway: GatewayStats,
    pub loss_tracker: LossBatchTracker,
    pub ping_tiers: TieredPingTracker,
    pub thresholds: PingThresholds,
    pub interval: IntervalTracker,
    /// Set by pinger when auto-stop fires; GUI checks and runs auto-export
    pub auto_export_pending: bool,
    /// Set by pinger when a new loss batch starts; GUI checks and shows notification
    pub notify_loss_pending: bool,
    /// Whether loss event notifications are enabled
    pub notify_loss_enabled: bool,
    /// Modem health-check and struggle-detection state
    pub modem: ModemState,
    /// Thread alive timestamps
    pub heartbeats: ThreadHeartbeats,
}

impl PingState {
    pub fn new(config: PingConfig) -> Self {
        Self {
            config,
            running: false,
            results: VecDeque::with_capacity(MAX_RESULTS),
            log_entries: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            interval_reports: VecDeque::with_capacity(256),
            all_latencies: VecDeque::with_capacity(MAX_LATENCIES),
            total_sent: 0,
            total_received: 0,
            seq_counter: 0,
            config_changed: false,
            start_time: None,
            jitter: JitterTracker::new(),
            gateway: GatewayStats::new(),
            loss_tracker: LossBatchTracker::new(),
            ping_tiers: TieredPingTracker::new(),
            thresholds: PingThresholds::default(),
            interval: IntervalTracker::new(),
            auto_export_pending: false,
            notify_loss_pending: false,
            notify_loss_enabled: false,
            modem: ModemState::new(),
            heartbeats: ThreadHeartbeats::new(),
        }
    }

    /// Record a ping result with jitter, loss batch, and tiered high-ping tracking.
    pub fn push_result(&mut self, result: PingResult) {
        let new_loss_batch = self.loss_tracker.record(result.success);
        if new_loss_batch && self.notify_loss_enabled {
            self.notify_loss_pending = true;
        }

        // Track external loss batches that start while gateway is healthy
        if new_loss_batch
            && self.gateway.enabled
            && self.gateway.total_sent >= 10
            && self.gateway.recent_loss_pct() < 1.0
        {
            self.modem.struggle_events.push_back(Instant::now());
            if self.modem.struggle_events.len() > 32 {
                self.modem.struggle_events.pop_front();
            }
        }

        self.ping_tiers.record(result.latency_ms, &self.thresholds);

        if let Some(lat) = result.latency_ms {
            self.jitter.record(lat);
            if self.all_latencies.len() >= MAX_LATENCIES {
                self.all_latencies.pop_front();
            }
            self.all_latencies.push_back(lat);
        }
        if self.results.len() >= MAX_RESULTS {
            self.results.pop_front();
        }
        self.results.push_back(result);
    }

    pub fn push_log(&mut self, message: String, latency_ms: Option<f64>, success: bool) {
        let entry = PingLogEntry { message, latency_ms, success };
        if self.log_entries.len() >= MAX_LOG_ENTRIES {
            self.log_entries.pop_front();
        }
        self.log_entries.push_back(entry);
    }

    pub fn packet_loss_pct(&self) -> f64 {
        if self.total_sent == 0 { return 0.0; }
        let lost = self.total_sent - self.total_received;
        (lost as f64 / self.total_sent as f64) * 100.0
    }

    /// Recent loss percentage over the last ~30 pings
    pub fn recent_loss_pct(&self) -> f64 {
        let window: Vec<bool> = self.results.iter().rev().take(30).map(|r| r.success).collect();
        if window.is_empty() { return 0.0; }
        let failed = window.iter().filter(|&&s| !s).count();
        (failed as f64 / window.len() as f64) * 100.0
    }

    /// Generate a text diagnosis for reports (mirrors monitor tab logic)
    pub fn diagnosis_text(&self) -> &'static str {
        let gw_loss = self.gateway.recent_loss_pct();
        let ext_loss = self.recent_loss_pct();
        let modem_http_failed = matches!(&self.modem.http_status, ModemHttpStatus::Failed(_));
        let modem_struggling = self.modem.struggle_count(self.modem.struggle_window_mins) >= 3;

        if self.gateway.total_sent == 0 || self.total_sent == 0 {
            "Insufficient data"
        } else if gw_loss > 2.0 {
            "Local network issue — packet loss between device and router"
        } else if modem_http_failed && ext_loss > 2.0 {
            "Modem CPU struggling — HTTP check failed with external packet loss"
        } else if modem_struggling {
            "Modem may be struggling — repeated external loss while gateway is healthy"
        } else if modem_http_failed {
            "Modem web UI unreachable — HTTP check failed but traffic is flowing"
        } else if ext_loss > 2.0 {
            "ISP or routing issue — local network stable but external target has loss"
        } else {
            "No issues detected"
        }
    }

    pub fn avg_latency(&self) -> f64 {
        if self.all_latencies.is_empty() { return 0.0; }
        self.all_latencies.iter().sum::<f64>() / self.all_latencies.len() as f64
    }

    pub fn min_latency(&self) -> f64 {
        self.all_latencies.iter().cloned().fold(f64::MAX, f64::min)
    }

    pub fn max_latency(&self) -> f64 {
        self.all_latencies.iter().cloned().fold(0.0_f64, f64::max)
    }

    pub fn elapsed_display(&self) -> String {
        let Some(start) = self.start_time else { return "-".to_string() };
        let total_secs = start.elapsed().as_secs();
        format!("{}m {}s", total_secs / 60, total_secs % 60)
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.start_time.map(|start| start.elapsed().as_secs_f64()).unwrap_or(0.0)
    }

    /// Flush the current partial interval as a report. Single-pass computation.
    pub fn flush_partial_report(&mut self) {
        if self.interval.results.is_empty() {
            return;
        }
        let now = chrono::Local::now().naive_local();
        let report = build_interval_report(
            &self.interval.results,
            self.interval.start_time.unwrap_or(now),
            now,
        );
        self.interval_reports.push_back(report);
        if self.interval_reports.len() > 256 {
            self.interval_reports.pop_front();
        }
        self.interval.reset();
    }

    pub fn reset_data(&mut self) {
        self.results.clear();
        self.log_entries.clear();
        self.interval_reports.clear();
        self.all_latencies.clear();
        self.total_sent = 0;
        self.total_received = 0;
        self.seq_counter = 0;
        self.config_changed = false;
        self.start_time = None;
        self.jitter.reset();
        self.gateway.reset();
        self.loss_tracker.reset();
        self.ping_tiers.reset();
        self.interval.reset();
        self.auto_export_pending = false;
        self.notify_loss_pending = false;
        self.modem.reset_runtime();
    }

    /// Stop the test, flushing any partial interval report.
    pub fn stop(&mut self) {
        self.flush_partial_report();
        self.running = false;
    }

    /// Start a new test, resetting all data.
    pub fn start(&mut self) {
        self.reset_data();
        self.running = true;
    }
}

/// Build an interval report from a slice of results in a single pass.
pub fn build_interval_report(
    results: &[PingResult],
    start_time: chrono::NaiveDateTime,
    end_time: chrono::NaiveDateTime,
) -> IntervalReport {
    let mut successful: u64 = 0;
    let mut failed: u64 = 0;
    let mut lat_sum: f64 = 0.0;
    let mut lat_count: u64 = 0;
    let mut lat_min: f64 = f64::MAX;
    let mut lat_max: f64 = 0.0;
    let mut loss_events: u64 = 0;
    let mut in_loss_batch = false;

    for result in results {
        if result.success {
            successful += 1;
            in_loss_batch = false;
        } else {
            failed += 1;
            if !in_loss_batch {
                loss_events += 1;
                in_loss_batch = true;
            }
        }
        if let Some(lat) = result.latency_ms {
            lat_sum += lat;
            lat_count += 1;
            if lat < lat_min { lat_min = lat; }
            if lat > lat_max { lat_max = lat; }
        }
    }

    let total = successful + failed;
    IntervalReport {
        start_time,
        end_time,
        total_pings: total,
        successful,
        failed,
        packet_loss_pct: if total > 0 { (failed as f64 / total as f64) * 100.0 } else { 0.0 },
        avg_latency_ms: if lat_count > 0 { lat_sum / lat_count as f64 } else { 0.0 },
        min_latency_ms: if lat_min == f64::MAX { 0.0 } else { lat_min },
        max_latency_ms: lat_max,
        loss_events,
    }
}

/// Thread-safe shared state handle.
pub type SharedState = Arc<Mutex<PingState>>;

/// Signal for cleanly shutting down background threads.
pub type ShutdownSignal = Arc<std::sync::atomic::AtomicBool>;

/// Create a new shared state wrapped in Arc<Mutex<>> for cross-thread access
pub fn new_shared_state(config: PingConfig) -> SharedState {
    Arc::new(Mutex::new(PingState::new(config)))
}

/// Create a new shutdown signal
pub fn new_shutdown_signal() -> ShutdownSignal {
    Arc::new(std::sync::atomic::AtomicBool::new(false))
}

/// Lock shared state, recovering from poison with a warning.
pub fn lock_state(state: &SharedState) -> std::sync::MutexGuard<'_, PingState> {
    state.lock().unwrap_or_else(|poison| {
        eprintln!("WARNING: Mutex poisoned, recovering with potentially inconsistent state");
        poison.into_inner()
    })
}
