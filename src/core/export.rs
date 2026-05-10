//! # Export — CSV and JSON file writers
//!
//! Writes ping results and interval reports to files next to the executable.

use std::io::Write;

use crate::core::state::PingState;

/// Get the directory to write export files to (same directory as the executable)
pub fn export_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

/// Write ping results and interval reports to a CSV file
pub fn write_csv(path: &std::path::Path, state: &PingState) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    writeln!(file, "seq,success,latency_ms,timestamp")?;
    for result in &state.results {
        writeln!(file, "{},{},{},{}",
            result.seq, result.success,
            result.latency_ms.map(|lat| format!("{:.1}", lat)).unwrap_or_default(),
            result.timestamp.format("%Y-%m-%d %H:%M:%S"),
        )?;
    }
    writeln!(file)?;
    writeln!(file, "--- Interval Reports ---")?;
    writeln!(file, "start,end,total,ok,fail,loss%,avg_ms,min_ms,max_ms")?;
    for report in &state.interval_reports {
        writeln!(file, "{},{},{},{},{},{:.1},{:.1},{:.1},{:.1}",
            report.start_time.format("%H:%M:%S"), report.end_time.format("%H:%M:%S"),
            report.total_pings, report.successful, report.failed, report.packet_loss_pct,
            report.avg_latency_ms, report.min_latency_ms, report.max_latency_ms,
        )?;
    }
    Ok(())
}

/// Write ping results and interval reports to a JSON file
pub fn write_json(path: &std::path::Path, state: &PingState) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    let min = state.min_latency();

    writeln!(file, "{{")?;
    writeln!(file, "  \"summary\": {{")?;
    writeln!(file, "    \"target\": \"{}\",", state.config.target)?;
    writeln!(file, "    \"total_sent\": {},", state.total_sent)?;
    writeln!(file, "    \"total_received\": {},", state.total_received)?;
    writeln!(file, "    \"packet_loss_pct\": {:.1},", state.packet_loss_pct())?;
    writeln!(file, "    \"avg_latency_ms\": {:.1},", state.avg_latency())?;
    writeln!(file, "    \"min_latency_ms\": {:.1},", if min == f64::MAX { 0.0 } else { min })?;
    writeln!(file, "    \"max_latency_ms\": {:.1},", state.max_latency())?;
    writeln!(file, "    \"avg_jitter_ms\": {:.1}", state.avg_jitter())?;
    writeln!(file, "  }},")?;

    write_json_results(&mut file, state)?;
    write_json_reports(&mut file, state)?;

    writeln!(file, "}}")
}

fn write_json_results(file: &mut std::fs::File, state: &PingState) -> std::io::Result<()> {
    writeln!(file, "  \"results\": [")?;
    for (idx, result) in state.results.iter().enumerate() {
        let comma = if idx + 1 < state.results.len() { "," } else { "" };
        writeln!(file, "    {{\"seq\": {}, \"success\": {}, \"latency_ms\": {}, \"timestamp\": \"{}\"}}{}",
            result.seq, result.success,
            result.latency_ms.map(|lat| format!("{:.1}", lat)).unwrap_or("null".into()),
            result.timestamp.format("%Y-%m-%d %H:%M:%S"), comma,
        )?;
    }
    writeln!(file, "  ],")
}

fn write_json_reports(file: &mut std::fs::File, state: &PingState) -> std::io::Result<()> {
    writeln!(file, "  \"interval_reports\": [")?;
    for (idx, report) in state.interval_reports.iter().enumerate() {
        let comma = if idx + 1 < state.interval_reports.len() { "," } else { "" };
        writeln!(file, "    {{\"start\": \"{}\", \"end\": \"{}\", \"total\": {}, \"ok\": {}, \"fail\": {}, \"loss_pct\": {:.1}, \"avg_ms\": {:.1}, \"min_ms\": {:.1}, \"max_ms\": {:.1}}}{}",
            report.start_time.format("%H:%M:%S"), report.end_time.format("%H:%M:%S"),
            report.total_pings, report.successful, report.failed, report.packet_loss_pct,
            report.avg_latency_ms, report.min_latency_ms, report.max_latency_ms, comma,
        )?;
    }
    writeln!(file, "  ]")
}
