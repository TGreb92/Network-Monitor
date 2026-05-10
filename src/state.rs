//! # Shared State Types
//!
//! Defines all data structures shared between the pinger thread and the GUI.
//! Uses `RwLock` instead of `Mutex` because the GUI reads far more frequently
//! than the pinger writes, allowing concurrent read access.
//!
//! All collections are bounded using `VecDeque` with `pop_front()` eviction
//! to prevent unbounded memory growth during long-running sessions.

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Maximum number of ping results to retain (at 1 ping/sec, ~2 hours of data)
pub const MAX_RESULTS: usize = 7200;
/// Maximum number of console log entries to retain
pub const MAX_LOG_ENTRIES: usize = 2000;
/// Maximum number of latency data points for the chart (matches MAX_RESULTS)
pub const MAX_LATENCIES: usize = 7200;

/// User-configurable ping parameters
#[derive(Clone, Debug)]
pub struct PingConfig {
    /// Hostname or IP address to ping (e.g. "8.8.8.8", "google.com")
    pub target: String,
    /// Ping timeout in milliseconds — how long to wait for a reply
    pub timeout_ms: u32,
    /// How often (in seconds) to generate an interval summary report
    pub interval_secs: u64,
}

impl Default for PingConfig {
    fn default() -> Self {
        Self {
            target: "8.8.8.8".to_string(),
            timeout_ms: 2000,
            interval_secs: 60,
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
/// All mutable access goes through `RwLock` — the pinger acquires a write lock
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
        }
    }

    /// Record a ping result. Evicts the oldest entry if the collection is at capacity.
    /// Also tracks the latency separately for chart rendering.
    pub fn push_result(&mut self, result: PingResult) {
        // Track latency in a separate bounded deque for efficient chart rendering
        if let Some(lat) = result.latency_ms {
            if self.all_latencies.len() >= MAX_LATENCIES {
                self.all_latencies.pop_front();
            }
            self.all_latencies.push_back(lat);
        }
        // Evict oldest result if at capacity
        if self.results.len() >= MAX_RESULTS {
            self.results.pop_front();
        }
        self.results.push_back(result);
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
}

/// Thread-safe shared state handle. Uses `RwLock` for concurrent read access.
pub type SharedState = Arc<RwLock<PingState>>;

/// Create a new shared state wrapped in Arc<RwLock<>> for cross-thread access
pub fn new_shared_state(config: PingConfig) -> SharedState {
    Arc::new(RwLock::new(PingState::new(config)))
}
