//! # Latency Tiers
//!
//! 3-tier latency classification (Elevated/High/Critical) with batch
//! tracking and notification state for each tier.

/// Severity tier for ping events, ordered from lowest to highest.
/// Used for latency classification, console coloring, and notification priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PingTier {
    Normal,
    Elevated,
    High,
    Critical,
    Loss,
}

/// The 3 configurable latency tiers (excludes Normal and Loss)
const LATENCY_TIERS: [PingTier; 3] = [PingTier::Elevated, PingTier::High, PingTier::Critical];

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
            Self::Loss => "Loss",
        }
    }

    /// RGB color associated with this tier for UI rendering
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Self::Normal => [180, 220, 180],    // green
            Self::Elevated => [200, 200, 50],   // yellow
            Self::High => [255, 150, 50],       // orange
            Self::Critical => [255, 80, 80],    // red
            Self::Loss => [255, 100, 100],      // bright red
        }
    }

    /// Get the threshold value for this tier from the config
    pub fn threshold(self, t: &PingThresholds) -> f64 {
        match self {
            Self::Elevated => t.elevated_ms,
            Self::High => t.high_ms,
            Self::Critical => t.critical_ms,
            _ => 0.0,
        }
    }

    /// Index for array-based storage (only latency tiers)
    fn index(self) -> usize {
        match self {
            Self::Elevated => 0,
            Self::High => 1,
            Self::Critical => 2,
            _ => 0,
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

/// Per-tier state: batch tracker + notification flags
#[derive(Clone)]
struct TierState {
    tracker: HighPingBatchTracker,
    notify_pending: bool,
    notify_enabled: bool,
}

impl TierState {
    fn new() -> Self {
        Self {
            tracker: HighPingBatchTracker::new(),
            notify_pending: false,
            notify_enabled: false,
        }
    }
}

/// Tracks all 3 latency tiers with batch detection and notification state.
#[derive(Clone)]
pub struct TieredPingTracker {
    tiers: [TierState; 3],
}

impl TieredPingTracker {
    pub fn new() -> Self {
        Self {
            tiers: [TierState::new(), TierState::new(), TierState::new()],
        }
    }

    /// Get the batch count for a specific tier
    pub fn count(&self, tier: PingTier) -> u64 {
        self.tiers[tier.index()].tracker.count
    }

    /// Set whether notifications are enabled for a tier
    pub fn set_enabled(&mut self, tier: PingTier, enabled: bool) {
        self.tiers[tier.index()].notify_enabled = enabled;
    }

    /// Record a latency against all 3 tiers. Sets pending flags for new batches.
    pub fn record(&mut self, latency_ms: Option<f64>, thresholds: &PingThresholds) {
        for &tier in &LATENCY_TIERS {
            let state = &mut self.tiers[tier.index()];
            if state.tracker.record(latency_ms, tier.threshold(thresholds)) && state.notify_enabled {
                state.notify_pending = true;
            }
        }
    }

    pub fn reset(&mut self) {
        for state in &mut self.tiers {
            state.tracker.reset();
            state.notify_pending = false;
        }
    }

    /// Clear all pending flags without resetting trackers
    pub fn reset_pending(&mut self) {
        for state in &mut self.tiers {
            state.notify_pending = false;
        }
    }

    /// Return the highest-severity tier that has a pending notification,
    /// without clearing the flag. Returns None if nothing pending.
    pub fn highest_pending(&self) -> Option<PingTier> {
        LATENCY_TIERS.iter().rev()
            .find(|&&tier| {
                let state = &self.tiers[tier.index()];
                state.notify_pending && state.notify_enabled
            })
            .copied()
    }

    /// Clear the pending flag for a specific tier (and all lower tiers)
    pub fn clear_pending_up_to(&mut self, tier: PingTier) {
        for &t in &LATENCY_TIERS {
            if t <= tier {
                self.tiers[t.index()].notify_pending = false;
            }
        }
    }

    /// Drain all pending tier notifications. Returns (tier, count, threshold_ms)
    /// for each tier that has a pending notification, clearing the flags.
    pub fn drain_pending(&mut self, thresholds: &PingThresholds) -> Vec<(PingTier, u64, u32)> {
        let mut pending = Vec::new();
        for &tier in &LATENCY_TIERS {
            let state = &mut self.tiers[tier.index()];
            if state.notify_pending && state.notify_enabled {
                state.notify_pending = false;
                pending.push((tier, state.tracker.count, tier.threshold(thresholds) as u32));
            }
        }
        pending
    }
}
