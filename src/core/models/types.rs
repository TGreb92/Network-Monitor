//! # Data Types
//!
//! Core data structures for ping results, log entries, interval reports,
//! and ping configuration.

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
    /// Round-trip latency if available (for coloring high-ping entries)
    pub latency_ms: Option<f64>,
    /// Whether this ping was successful
    pub success: bool,
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
