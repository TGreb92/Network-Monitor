//! # Latency Tiers
//!
//! 3-tier latency classification (Elevated/High/Critical) with batch
//! tracking and notification state for each tier.

/// Severity tier for high-latency pings
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PingTier {
    Normal,
    Elevated,
    High,
    Critical,
}

impl PingTier {
    /// Classify a latency against the 3 tier thresholds
    pub fn classify(latency_ms: Option<f64>, thresholds: &PingThresholds) -> Self {
        let Some(lat) = latency_ms else { return Self::Normal };
        if lat >= thresholds.critical_ms {
            Self::Critical
        } else if lat >= thresholds.high_ms {
            Self::High
        } else if lat >= thresholds.elevated_ms {
            Self::Elevated
        } else {
            Self::Normal
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Elevated => "Elevated",
            Self::High => "High",
            Self::Critical => "Critical",
        }
    }
}

/// Configurable thresholds for the 3 ping severity tiers (in ms)
#[derive(Clone, Debug)]
pub struct PingThresholds {
    pub elevated_ms: f64,
    pub high_ms: f64,
    pub critical_ms: f64,
}

impl Default for PingThresholds {
    fn default() -> Self {
        Self { elevated_ms: 100.0, high_ms: 200.0, critical_ms: 500.0 }
    }
}

/// Tracks batch events for a single latency threshold
#[derive(Clone)]
pub struct HighPingBatchTracker {
    pub count: u64,
    pub in_batch: bool,
}

impl HighPingBatchTracker {
    pub fn new() -> Self {
        Self { count: 0, in_batch: false }
    }

    /// Returns true if a NEW batch just started (latency crossed above threshold).
    pub fn record(&mut self, latency_ms: Option<f64>, threshold_ms: f64) -> bool {
        let is_high = latency_ms.is_some_and(|lat| lat >= threshold_ms);
        if is_high && !self.in_batch {
            self.count += 1;
            self.in_batch = true;
            true
        } else {
            if !is_high {
                self.in_batch = false;
            }
            false
        }
    }

    pub fn reset(&mut self) {
        self.count = 0;
        self.in_batch = false;
    }
}

/// Per-tier batch tracker and notification state
#[derive(Clone)]
pub struct TieredPingTracker {
    pub elevated: HighPingBatchTracker,
    pub high: HighPingBatchTracker,
    pub critical: HighPingBatchTracker,
    pub notify_elevated_pending: bool,
    pub notify_high_pending: bool,
    pub notify_critical_pending: bool,
    pub notify_elevated_enabled: bool,
    pub notify_high_enabled: bool,
    pub notify_critical_enabled: bool,
}

impl TieredPingTracker {
    pub fn new() -> Self {
        Self {
            elevated: HighPingBatchTracker::new(),
            high: HighPingBatchTracker::new(),
            critical: HighPingBatchTracker::new(),
            notify_elevated_pending: false,
            notify_high_pending: false,
            notify_critical_pending: false,
            notify_elevated_enabled: false,
            notify_high_enabled: false,
            notify_critical_enabled: false,
        }
    }

    /// Record a latency against all 3 tiers. Sets pending flags for new batches.
    pub fn record(&mut self, latency_ms: Option<f64>, thresholds: &PingThresholds) {
        if self.elevated.record(latency_ms, thresholds.elevated_ms) && self.notify_elevated_enabled {
            self.notify_elevated_pending = true;
        }
        if self.high.record(latency_ms, thresholds.high_ms) && self.notify_high_enabled {
            self.notify_high_pending = true;
        }
        if self.critical.record(latency_ms, thresholds.critical_ms) && self.notify_critical_enabled {
            self.notify_critical_pending = true;
        }
    }

    pub fn reset(&mut self) {
        self.elevated.reset();
        self.high.reset();
        self.critical.reset();
        self.notify_elevated_pending = false;
        self.notify_high_pending = false;
        self.notify_critical_pending = false;
    }

    /// Drain all pending tier notifications. Returns (label, count, threshold_ms)
    /// for each tier that has a pending notification, clearing the flags.
    pub fn drain_pending(&mut self, thresholds: &PingThresholds) -> Vec<(&'static str, u64, u32)> {
        let mut pending = Vec::new();
        if self.notify_elevated_pending && self.notify_elevated_enabled {
            self.notify_elevated_pending = false;
            pending.push(("Elevated Ping", self.elevated.count, thresholds.elevated_ms as u32));
        }
        if self.notify_high_pending && self.notify_high_enabled {
            self.notify_high_pending = false;
            pending.push(("High Ping", self.high.count, thresholds.high_ms as u32));
        }
        if self.notify_critical_pending && self.notify_critical_enabled {
            self.notify_critical_pending = false;
            pending.push(("Critical Ping", self.critical.count, thresholds.critical_ms as u32));
        }
        pending
    }
}
