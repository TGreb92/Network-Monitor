//! # JSON serialization types
//!
//! Shared serde types used by both JSON export (write) and import (read).

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct JsonExport {
    pub test_info: JsonTestInfo,
    pub summary: JsonSummary,
    pub gateway: Option<JsonGateway>,
    pub results: Vec<JsonResult>,
    pub interval_reports: Vec<JsonIntervalReport>,
    pub console_log: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct JsonTestInfo {
    pub generated: String,
    pub target: String,
    pub duration: String,
    pub ping_interval_ms: u64,
    pub ping_timeout_ms: u32,
}

#[derive(Serialize, Deserialize)]
pub struct JsonSummary {
    pub total_sent: u64,
    pub total_received: u64,
    pub packets_lost: u64,
    pub packet_loss_pct: f64,
    pub loss_events: u64,
    pub avg_latency_ms: f64,
    pub min_latency_ms: f64,
    pub max_latency_ms: f64,
    pub avg_jitter_ms: f64,
}

#[derive(Serialize, Deserialize)]
pub struct JsonGateway {
    pub ip: String,
    pub total_sent: u64,
    pub total_received: u64,
    pub packet_loss_pct: f64,
    pub avg_latency_ms: f64,
    pub avg_jitter_ms: f64,
    pub latencies: Vec<f64>,
}

#[derive(Serialize, Deserialize)]
pub struct JsonResult {
    pub seq: u64,
    pub success: bool,
    pub latency_ms: Option<f64>,
    pub timestamp: String,
    pub elapsed_secs: f64,
}

#[derive(Serialize, Deserialize)]
pub struct JsonIntervalReport {
    pub start: String,
    pub end: String,
    pub total: u64,
    pub ok: u64,
    pub fail: u64,
    pub loss_pct: f64,
    pub loss_events: u64,
    pub avg_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
}
