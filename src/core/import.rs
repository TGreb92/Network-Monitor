//! # Import - JSON reader and state reconstruction
//!
//! Reads a previously exported JSON file and reconstructs a PingState
//! for reviewing past sessions (read-only, not running).

use crate::core::json_types::*;
use crate::core::state::{
    PingState, PingResult, PingLogEntry, IntervalReport,
    PingConfig, GatewayStats, LossBatchTracker, MAX_LATENCIES,
};

/// Import a previously exported JSON file and reconstruct PingState for viewing.
pub fn read_json(path: &std::path::Path) -> Result<PingState, String> {
    let file = std::fs::File::open(path)
        .map_err(|err| format!("Failed to open file: {}", err))?;
    let reader = std::io::BufReader::new(file);
    let export: JsonExport = serde_json::from_reader(reader)
        .map_err(|err| format!("Failed to parse JSON: {}", err))?;

    let config = PingConfig {
        target: export.test_info.target,
        timeout_ms: export.test_info.ping_timeout_ms,
        interval_secs: 60,
        ping_interval_ms: export.test_info.ping_interval_ms,
        duration_secs: 0,
        use_tcp: false,
        tcp_port: 443,
    };

    let mut state = PingState::new(config);
    state.total_sent = export.summary.total_sent;
    state.total_received = export.summary.total_received;
    state.loss_tracker = LossBatchTracker {
        count: export.summary.loss_events,
        in_batch: false,
    };

    for json_result in &export.results {
        let timestamp = chrono::NaiveDateTime::parse_from_str(
            &json_result.timestamp, "%Y-%m-%d %H:%M:%S"
        ).unwrap_or_default();

        let result = PingResult {
            seq: json_result.seq,
            success: json_result.success,
            latency_ms: json_result.latency_ms,
            timestamp,
            elapsed_secs: json_result.elapsed_secs,
        };

        if let Some(latency) = json_result.latency_ms {
            state.jitter.record(latency);
            if state.all_latencies.len() >= MAX_LATENCIES {
                state.all_latencies.pop_front();
            }
            state.all_latencies.push_back(latency);
        }

        state.results.push_back(result);
    }

    state.seq_counter = export.results.last().map(|res| res.seq).unwrap_or(0);

    for json_report in &export.interval_reports {
        let start_time = chrono::NaiveTime::parse_from_str(&json_report.start, "%H:%M:%S")
            .unwrap_or_default();
        let end_time = chrono::NaiveTime::parse_from_str(&json_report.end, "%H:%M:%S")
            .unwrap_or_default();
        let today = chrono::Local::now().date_naive();

        state.interval_reports.push_back(IntervalReport {
            start_time: today.and_time(start_time),
            end_time: today.and_time(end_time),
            total_pings: json_report.total,
            successful: json_report.ok,
            failed: json_report.fail,
            packet_loss_pct: json_report.loss_pct,
            avg_latency_ms: json_report.avg_ms,
            min_latency_ms: json_report.min_ms,
            max_latency_ms: json_report.max_ms,
            loss_events: json_report.loss_events,
        });
    }

    if let Some(gw) = &export.gateway {
        state.gateway = GatewayStats::new();
        state.gateway.ip = Some(gw.ip.clone());
        state.gateway.enabled = true;
        state.gateway.total_sent = gw.total_sent;
        state.gateway.total_received = gw.total_received;
        for latency in &gw.latencies {
            state.gateway.jitter.record(*latency);
            if state.gateway.all_latencies.len() >= MAX_LATENCIES {
                state.gateway.all_latencies.pop_front();
            }
            state.gateway.all_latencies.push_back(*latency);
        }
    }

    for message in &export.console_log {
        state.log_entries.push_back(PingLogEntry {
            message: message.clone(),
            latency_ms: None,
            success: true,
        });
    }

    Ok(state)
}
