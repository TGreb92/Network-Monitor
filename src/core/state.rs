//! # Shared State Types
//!
//! Defines all data structures shared between the pinger thread and the GUI.
//! Uses `Arc<Mutex>` for thread-safe shared state. Mutex is simpler and cheaper
//! than RwLock at our low throughput (~2 reads/sec, ~1 write/sec).
//!
//! All collections are bounded using `VecDeque` with `pop_front()` eviction
//! to prevent unbounded memory growth during long-running sessions.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Maximum number of ping results to retain (at 1 ping/sec, ~2 hours of data)
pub const MAX_RESULTS: usize = 7200;
/// Maximum number of console log entries to retain
pub const MAX_LOG_ENTRIES: usize = 2000;
/// Maximum number of latency data points for the chart (matches MAX_RESULTS)
pub const MAX_LATENCIES: usize = 7200;
/// Maximum number of jitter data points to retain
pub const MAX_JITTER: usize = 7200;

/// User-configurable ping parameters
#[derive(Clone, Debug)]
pub struct PingConfig {
    /// Hostname or IP address to ping (e.g. "8.8.8.8", "google.com")
    pub target: String,
    /// Ping timeout in milliseconds - how long to wait for a reply
    pub timeout_ms: u32,
    /// How often (in seconds) to generate an interval summary report
    pub interval_secs: u64,
    /// Milliseconds between consecutive pings (default 1000ms = 1 ping/sec)
    pub ping_interval_ms: u64,
    /// Test duration in seconds. 0 = unlimited.
    pub duration_secs: u64,
}

impl PingConfig {
    /// Create a PingConfig from a saved config loaded from disk
    pub fn from_saved(saved: &crate::core::config::SavedConfig) -> Self {
        let target = saved.presets
            .get(saved.selected_preset)
            .map(|preset| preset.host.clone())
            .unwrap_or_else(|| "8.8.8.8".to_string());
        Self {
            target,
            timeout_ms: saved.timeout_ms,
            interval_secs: saved.interval_secs,
            ping_interval_ms: saved.ping_interval_ms,
            duration_secs: saved.duration_mins * 60,
        }
    }
}

/// A single ping attempt and its outcome
#[derive(Clone, Debug)]
pub struct PingResult {
    /// Monotonically increasing sequence number
    pub seq: u64,
    /// Whether a reply was received within the timeout
    pub success: bool,
    /// Round-trip time in ms (None if the ping timed out)
    pub latency_ms: Option<f64>,
    /// Local timestamp when this ping was recorded
    pub timestamp: chrono::NaiveDateTime,
    /// Seconds elapsed since monitoring started (X-axis for chart)
    pub elapsed_secs: f64,
}

/// A single line in the console log
#[derive(Clone, Debug)]
pub struct PingLogEntry {
    /// Formatted log message (e.g. "[12:34:56] #42 Reply from 8.8.8.8: time=15ms")
    pub message: String,
}

/// Summary statistics for a reporting interval (e.g. every 60 seconds)
#[derive(Clone, Debug)]
pub struct IntervalReport {
    pub start_time: chrono::NaiveDateTime,
    pub end_time: chrono::NaiveDateTime,
    pub total_pings: u64,
    pub successful: u64,
    pub failed: u64,
    pub packet_loss_pct: f64,
    pub avg_latency_ms: f64,
    pub min_latency_ms: f64,
    pub max_latency_ms: f64,
    /// Number of distinct loss batches within this interval
    pub loss_events: u64,
}

/// Tracks jitter (latency variation between consecutive pings)
pub struct JitterTracker {
    pub last_latency: Option<f64>,
    pub values: VecDeque<f64>,
}

impl JitterTracker {
    pub fn new() -> Self {
        Self {
            last_latency: None,
            values: VecDeque::with_capacity(MAX_JITTER),
        }
    }

    /// Record a latency and compute jitter from the previous value
    pub fn record(&mut self, latency: f64) {
        if let Some(prev) = self.last_latency {
            let jitter = (latency - prev).abs();
            if self.values.len() >= MAX_JITTER {
                self.values.pop_front();
            }
            self.values.push_back(jitter);
        }
        self.last_latency = Some(latency);
    }

    pub fn avg(&self) -> f64 {
        if self.values.is_empty() { return 0.0; }
        self.values.iter().sum::<f64>() / self.values.len() as f64
    }

    pub fn reset(&mut self) {
        self.last_latency = None;
        self.values.clear();
    }
}

/// Gateway ping statistics
pub struct GatewayStats {
    pub ip: Option<String>,
    pub enabled: bool,
    pub total_sent: u64,
    pub total_received: u64,
    pub all_latencies: VecDeque<f64>,
    pub jitter: JitterTracker,
}

impl GatewayStats {
    pub fn new() -> Self {
        Self {
            ip: None,
            enabled: false,
            total_sent: 0,
            total_received: 0,
            all_latencies: VecDeque::with_capacity(MAX_LATENCIES),
            jitter: JitterTracker::new(),
        }
    }

    pub fn push_result(&mut self, latency_ms: Option<f64>, success: bool) {
        self.total_sent += 1;
        if success {
            self.total_received += 1;
        }
        if let Some(lat) = latency_ms {
            self.jitter.record(lat);
            if self.all_latencies.len() >= MAX_LATENCIES {
                self.all_latencies.pop_front();
            }
            self.all_latencies.push_back(lat);
        }
    }

    pub fn packet_loss_pct(&self) -> f64 {
        if self.total_sent == 0 { return 0.0; }
        let lost = self.total_sent - self.total_received;
        (lost as f64 / self.total_sent as f64) * 100.0
    }

    pub fn avg_latency(&self) -> f64 {
        if self.all_latencies.is_empty() { return 0.0; }
        self.all_latencies.iter().sum::<f64>() / self.all_latencies.len() as f64
    }

    pub fn reset(&mut self) {
        self.total_sent = 0;
        self.total_received = 0;
        self.all_latencies.clear();
        self.jitter.reset();
    }
}

/// Tracks loss batch events (clusters of consecutive timeouts)
pub struct LossBatchTracker {
    pub count: u64,
    pub in_batch: bool,
}

impl LossBatchTracker {
    pub fn new() -> Self {
        Self { count: 0, in_batch: false }
    }

    pub fn record(&mut self, success: bool) {
        if success {
            self.in_batch = false;
        } else if !self.in_batch {
            self.count += 1;
            self.in_batch = true;
        }
    }

    pub fn reset(&mut self) {
        self.count = 0;
        self.in_batch = false;
    }
}

/// Tracks the current reporting interval and accumulates results
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

/// Central shared state accessed by both the pinger thread (writer) and the GUI (reader).
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
    pub interval: IntervalTracker,
    /// Set by pinger when auto-stop fires; GUI checks and runs auto-export
    pub auto_export_pending: bool,
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
            interval: IntervalTracker::new(),
            auto_export_pending: false,
        }
    }

    /// Record a ping result with jitter and loss batch tracking.
    pub fn push_result(&mut self, result: PingResult) {
        self.loss_tracker.record(result.success);

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

    pub fn push_log(&mut self, message: String) {
        let entry = PingLogEntry { message };
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
        self.interval.reset();
        self.auto_export_pending = false;
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

/// Create a new shared state wrapped in Arc<Mutex<>> for cross-thread access
pub fn new_shared_state(config: PingConfig) -> SharedState {
    Arc::new(Mutex::new(PingState::new(config)))
}
