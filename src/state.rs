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

/// Built-in target presets for quick selection
pub const PRESETS: &[(&str, &str)] = &[
    ("8.8.8.8", "Google DNS"),
    ("1.1.1.1", "Cloudflare DNS"),
    ("9.9.9.9", "Quad9 DNS"),
    ("208.67.222.222", "OpenDNS"),
];

/// User-configurable ping parameters
#[derive(Clone, Debug)]
pub struct PingConfig {
    /// Hostname or IP address to ping (e.g. "8.8.8.8", "google.com")
    pub target: String,
    /// Ping timeout in milliseconds — how long to wait for a reply
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
    pub fn from_saved(saved: &crate::config::SavedConfig) -> Self {
        Self {
            target: saved.target.clone(),
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
    /// Local timestamp for this log entry
    pub timestamp: chrono::NaiveDateTime,
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
}

/// Central shared state accessed by both the pinger thread (writer) and the GUI (reader).
///
/// All mutable access goes through `Mutex` — the pinger acquires the lock
/// once per ping, while the GUI acquires read locks every frame (~500ms).
pub struct PingState {
    /// Current ping configuration (target, timeout, interval)
    pub config: PingConfig,
    /// Whether the pinger is actively sending pings
    pub running: bool,
    /// Bounded ring buffer of individual ping results
    pub results: VecDeque<PingResult>,
    /// Bounded ring buffer of console log messages
    pub log_entries: VecDeque<PingLogEntry>,
    /// Bounded ring buffer of periodic interval reports
    pub interval_reports: VecDeque<IntervalReport>,
    /// Bounded ring buffer of latency values for the chart
    pub all_latencies: VecDeque<f64>,
    /// Lifetime count of pings sent
    pub total_sent: u64,
    /// Lifetime count of successful replies received
    pub total_received: u64,
    /// Next sequence number to assign
    pub seq_counter: u64,
    /// Monotonic clock reference for the current interval (for elapsed time)
    pub interval_start: Option<Instant>,
    /// Wall-clock start of the current interval (for display)
    pub interval_start_time: Option<chrono::NaiveDateTime>,
    /// Accumulator for pings within the current interval
    pub interval_results: Vec<PingResult>,
    /// Flag set by the GUI when config is updated; pinger resets interval tracking
    pub config_changed: bool,
    /// When monitoring started (for elapsed time calculation)
    pub start_time: Option<Instant>,

    // --- Jitter tracking ---
    /// Previous ping's latency, used to compute jitter (|current - previous|)
    pub last_latency: Option<f64>,
    /// Bounded ring buffer of jitter values for display/charting
    pub jitter_values: VecDeque<f64>,

    // --- Gateway / LAN health check ---
    /// Auto-detected or manually set default gateway IP
    pub gateway_ip: Option<String>,
    /// Whether gateway pinging is enabled
    pub gateway_enabled: bool,
    /// Gateway ping stats — sent count
    pub gw_total_sent: u64,
    /// Gateway ping stats — received count
    pub gw_total_received: u64,
    /// Gateway latency values for stats
    pub gw_all_latencies: VecDeque<f64>,
    /// Gateway jitter tracking
    pub gw_last_latency: Option<f64>,
    pub gw_jitter_values: VecDeque<f64>,

    // --- Export status message ---
    pub export_message: Option<(String, Instant)>,
}

impl PingState {
    /// Create a new PingState with pre-allocated bounded collections
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
            interval_start: None,
            interval_start_time: None,
            interval_results: Vec::new(),
            config_changed: false,
            start_time: None,
            last_latency: None,
            jitter_values: VecDeque::with_capacity(MAX_JITTER),
            gateway_ip: None,
            gateway_enabled: false,
            gw_total_sent: 0,
            gw_total_received: 0,
            gw_all_latencies: VecDeque::with_capacity(MAX_LATENCIES),
            gw_last_latency: None,
            gw_jitter_values: VecDeque::with_capacity(MAX_JITTER),
            export_message: None,
        }
    }

    /// Record a ping result. Evicts the oldest entry if the collection is at capacity.
    /// Also tracks the latency and jitter separately.
    pub fn push_result(&mut self, result: PingResult) {
        if let Some(lat) = result.latency_ms {
            // Compute jitter: absolute difference between consecutive latencies
            if let Some(prev) = self.last_latency {
                let jitter = (lat - prev).abs();
                if self.jitter_values.len() >= MAX_JITTER {
                    self.jitter_values.pop_front();
                }
                self.jitter_values.push_back(jitter);
            }
            self.last_latency = Some(lat);

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

    /// Record a gateway ping result. Tracks gateway-specific latency and jitter.
    pub fn push_gateway_result(&mut self, latency_ms: Option<f64>, success: bool) {
        self.gw_total_sent += 1;
        if success {
            self.gw_total_received += 1;
        }
        if let Some(lat) = latency_ms {
            if let Some(prev) = self.gw_last_latency {
                let jitter = (lat - prev).abs();
                if self.gw_jitter_values.len() >= MAX_JITTER {
                    self.gw_jitter_values.pop_front();
                }
                self.gw_jitter_values.push_back(jitter);
            }
            self.gw_last_latency = Some(lat);

            if self.gw_all_latencies.len() >= MAX_LATENCIES {
                self.gw_all_latencies.pop_front();
            }
            self.gw_all_latencies.push_back(lat);
        }
    }

    /// Append a message to the console log. Evicts the oldest entry if at capacity.
    pub fn push_log(&mut self, message: String) {
        let entry = PingLogEntry {
            timestamp: chrono::Local::now().naive_local(),
            message,
        };
        if self.log_entries.len() >= MAX_LOG_ENTRIES {
            self.log_entries.pop_front();
        }
        self.log_entries.push_back(entry);
    }

    /// Calculate overall packet loss as a percentage (0.0–100.0)
    pub fn packet_loss_pct(&self) -> f64 {
        if self.total_sent == 0 {
            return 0.0;
        }
        let lost = self.total_sent - self.total_received;
        (lost as f64 / self.total_sent as f64) * 100.0
    }

    /// Average latency across all recorded successful pings
    pub fn avg_latency(&self) -> f64 {
        if self.all_latencies.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.all_latencies.iter().sum();
        sum / self.all_latencies.len() as f64
    }

    /// Minimum latency across all recorded successful pings
    pub fn min_latency(&self) -> f64 {
        self.all_latencies
            .iter()
            .cloned()
            .fold(f64::MAX, f64::min)
    }

    /// Maximum latency across all recorded successful pings
    pub fn max_latency(&self) -> f64 {
        self.all_latencies
            .iter()
            .cloned()
            .fold(0.0_f64, f64::max)
    }

    /// Average jitter (mean of absolute latency differences between consecutive pings)
    pub fn avg_jitter(&self) -> f64 {
        if self.jitter_values.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.jitter_values.iter().sum();
        sum / self.jitter_values.len() as f64
    }

    /// Gateway packet loss percentage
    pub fn gw_packet_loss_pct(&self) -> f64 {
        if self.gw_total_sent == 0 {
            return 0.0;
        }
        let lost = self.gw_total_sent - self.gw_total_received;
        (lost as f64 / self.gw_total_sent as f64) * 100.0
    }

    /// Gateway average latency
    pub fn gw_avg_latency(&self) -> f64 {
        if self.gw_all_latencies.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.gw_all_latencies.iter().sum();
        sum / self.gw_all_latencies.len() as f64
    }

    /// Gateway average jitter
    pub fn gw_avg_jitter(&self) -> f64 {
        if self.gw_jitter_values.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.gw_jitter_values.iter().sum();
        sum / self.gw_jitter_values.len() as f64
    }

    /// Get elapsed time since monitoring started, formatted as "Xm Ys"
    pub fn elapsed_display(&self) -> String {
        match self.start_time {
            Some(start) => {
                let total_secs = start.elapsed().as_secs();
                let mins = total_secs / 60;
                let secs = total_secs % 60;
                format!("{}m {}s", mins, secs)
            }
            None => "—".to_string(),
        }
    }

    /// Get elapsed seconds since monitoring started
    pub fn elapsed_secs(&self) -> f64 {
        self.start_time
            .map(|start| start.elapsed().as_secs_f64())
            .unwrap_or(0.0)
    }

    /// Flush the current partial interval as a final report (called on stop)
    pub fn flush_partial_report(&mut self) {
        if self.interval_results.is_empty() {
            return;
        }
        let now = chrono::Local::now().naive_local();
        let start_time = self.interval_start_time.unwrap_or(now);
        let report = IntervalReport {
            start_time,
            end_time: now,
            total_pings: self.interval_results.len() as u64,
            successful: self.interval_results.iter().filter(|result| result.success).count() as u64,
            failed: self.interval_results.iter().filter(|result| !result.success).count() as u64,
            packet_loss_pct: {
                let total = self.interval_results.len() as f64;
                let failed = self.interval_results.iter().filter(|result| !result.success).count() as f64;
                if total > 0.0 { (failed / total) * 100.0 } else { 0.0 }
            },
            avg_latency_ms: {
                let lats: Vec<f64> = self.interval_results.iter().filter_map(|result| result.latency_ms).collect();
                if lats.is_empty() { 0.0 } else { lats.iter().sum::<f64>() / lats.len() as f64 }
            },
            min_latency_ms: {
                let min = self.interval_results.iter().filter_map(|result| result.latency_ms).fold(f64::MAX, f64::min);
                if min == f64::MAX { 0.0 } else { min }
            },
            max_latency_ms: self.interval_results.iter().filter_map(|result| result.latency_ms).fold(0.0_f64, f64::max),
        };
        self.interval_reports.push_back(report);
        if self.interval_reports.len() > 256 {
            self.interval_reports.pop_front();
        }
        self.interval_results.clear();
        self.interval_start = None;
        self.interval_start_time = None;
    }
}

/// Thread-safe shared state handle.
pub type SharedState = Arc<Mutex<PingState>>;

/// Create a new shared state wrapped in Arc<Mutex<>> for cross-thread access
pub fn new_shared_state(config: PingConfig) -> SharedState {
    Arc::new(Mutex::new(PingState::new(config)))
}
