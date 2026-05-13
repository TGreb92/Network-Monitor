//! # Trackers
//!
//! Jitter tracking, gateway statistics, and loss batch detection.

use std::collections::VecDeque;
use super::types::{MAX_LATENCIES, MAX_JITTER};

/// Tracks jitter (latency variation between consecutive pings)
#[derive(Clone)]
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

/// Maximum number of recent results to track for rolling loss calculation
const RECENT_WINDOW: usize = 30;

/// Gateway ping statistics
#[derive(Clone)]
pub struct GatewayStats {
    pub ip: Option<String>,
    pub enabled: bool,
    pub total_sent: u64,
    pub total_received: u64,
    pub all_latencies: VecDeque<f64>,
    pub jitter: JitterTracker,
    /// Rolling window of recent success/fail for diagnosis
    recent_results: VecDeque<bool>,
    /// Tracks gateway loss batches for notifications
    pub loss_tracker: LossBatchTracker,
    pub notify_loss_pending: bool,
    pub notify_loss_enabled: bool,
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
            recent_results: VecDeque::with_capacity(RECENT_WINDOW),
            loss_tracker: LossBatchTracker::new(),
            notify_loss_pending: false,
            notify_loss_enabled: false,
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
        if self.recent_results.len() >= RECENT_WINDOW {
            self.recent_results.pop_front();
        }
        self.recent_results.push_back(success);

        let new_batch = self.loss_tracker.record(success);
        if new_batch && self.notify_loss_enabled {
            self.notify_loss_pending = true;
        }
    }

    pub fn packet_loss_pct(&self) -> f64 {
        if self.total_sent == 0 { return 0.0; }
        let lost = self.total_sent - self.total_received;
        (lost as f64 / self.total_sent as f64) * 100.0
    }

    /// Recent loss percentage over the last ~30 pings
    pub fn recent_loss_pct(&self) -> f64 {
        if self.recent_results.is_empty() { return 0.0; }
        let failed = self.recent_results.iter().filter(|&&s| !s).count();
        (failed as f64 / self.recent_results.len() as f64) * 100.0
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
        self.recent_results.clear();
        self.loss_tracker.reset();
        self.notify_loss_pending = false;
    }
}

/// Tracks loss batch events (clusters of consecutive timeouts)
#[derive(Clone)]
pub struct LossBatchTracker {
    pub count: u64,
    pub in_batch: bool,
}

impl LossBatchTracker {
    pub fn new() -> Self {
        Self { count: 0, in_batch: false }
    }

    /// Records a ping result. Returns true if a NEW loss batch just started.
    pub fn record(&mut self, success: bool) -> bool {
        if success {
            self.in_batch = false;
            false
        } else if !self.in_batch {
            self.count += 1;
            self.in_batch = true;
            true
        } else {
            false
        }
    }

    pub fn reset(&mut self) {
        self.count = 0;
        self.in_batch = false;
    }
}
