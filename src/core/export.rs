//! # Export — CSV, JSON, ISP report, and console log writers
//!
//! Writes ping results, interval reports, human-readable ISP reports,
//! and raw console logs to files next to the executable.

use serde::Serialize;
use std::io::Write;

use crate::core::state::PingState;

/// Get the directory to write export files to (same directory as the executable)
pub fn export_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

/// Write a human-readable ISP report suitable for email/support tickets
pub fn write_isp_report(path: &std::path::Path, state: &PingState) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    let now = chrono::Local::now();
    let min = state.min_latency();
    let min_display = if min == f64::MAX { 0.0 } else { min };
    let lost = state.total_sent - state.total_received;
    let duration = state.elapsed_display();

    writeln!(file, "============================================================")?;
    writeln!(file, "  NETWORK CONNECTIVITY REPORT")?;
    writeln!(file, "============================================================")?;
    writeln!(file)?;
    writeln!(file, "Generated:     {}", now.format("%Y-%m-%d %H:%M:%S"))?;
    writeln!(file, "Target:        {}", state.config.target)?;
    writeln!(file, "Test duration: {}", duration)?;
    writeln!(file, "Ping interval: {} ms", state.config.ping_interval_ms)?;
    writeln!(file, "Ping timeout:  {} ms", state.config.timeout_ms)?;
    writeln!(file)?;

    writeln!(file, "--- SUMMARY ---")?;
    writeln!(file)?;
    writeln!(file, "  Pings sent:      {}", state.total_sent)?;
    writeln!(file, "  Pings received:  {}", state.total_received)?;
    writeln!(file, "  Packets lost:    {} ({:.1}%)", lost, state.packet_loss_pct())?;
    writeln!(file, "  Loss events:     {} (distinct connectivity drops)", state.loss_tracker.count)?;
    writeln!(file)?;
    writeln!(file, "  Avg latency:     {:.1} ms", state.avg_latency())?;
    writeln!(file, "  Min latency:     {:.1} ms", min_display)?;
    writeln!(file, "  Max latency:     {:.1} ms", state.max_latency())?;
    writeln!(file, "  Avg jitter:      {:.1} ms", state.jitter.avg())?;
    writeln!(file)?;

    // Gateway diagnosis if available
    if state.gateway.enabled && state.gateway.ip.is_some() {
        let gw_ip = state.gateway.ip.as_deref().unwrap_or("unknown");
        let gw_lost = state.gateway.total_sent - state.gateway.total_received;
        writeln!(file, "--- GATEWAY ANALYSIS ---")?;
        writeln!(file)?;
        writeln!(file, "  Gateway IP:      {}", gw_ip)?;
        writeln!(file, "  Gateway loss:    {} ({:.1}%)", gw_lost, state.gateway.packet_loss_pct())?;
        writeln!(file, "  Gateway avg:     {:.1} ms", state.gateway.avg_latency())?;
        writeln!(file, "  Gateway jitter:  {:.1} ms", state.gateway.jitter.avg())?;
        writeln!(file)?;

        let gw_loss = state.gateway.packet_loss_pct();
        let ext_loss = state.packet_loss_pct();
        if gw_loss > 2.0 {
            writeln!(file, "  DIAGNOSIS: Local network issue detected.")?;
            writeln!(file, "  Packet loss occurs between this device and the router.")?;
        } else if ext_loss > 2.0 {
            writeln!(file, "  DIAGNOSIS: ISP or routing issue detected.")?;
            writeln!(file, "  Local network is stable (gateway loss {:.1}%), but", gw_loss)?;
            writeln!(file, "  external target shows {:.1}% loss — the problem is", ext_loss)?;
            writeln!(file, "  between the router and the destination.")?;
        } else {
            writeln!(file, "  DIAGNOSIS: No issues detected. Both local and external")?;
            writeln!(file, "  connections are healthy.")?;
        }
        writeln!(file)?;
    }

    // Interval breakdown
    if !state.interval_reports.is_empty() {
        writeln!(file, "--- INTERVAL BREAKDOWN ---")?;
        writeln!(file)?;
        writeln!(file, "  {:<17} {:>5} {:>5} {:>5} {:>6} {:>6} {:>7} {:>7} {:>7}",
            "Time", "Sent", "OK", "Fail", "Loss%", "Evts", "Avg ms", "Min ms", "Max ms")?;
        writeln!(file, "  {}", "-".repeat(75))?;

        for report in &state.interval_reports {
            writeln!(file, "  {:<17} {:>5} {:>5} {:>5} {:>5.1}% {:>6} {:>7.1} {:>7.1} {:>7.1}",
                format!("{}-{}", report.start_time.format("%H:%M:%S"), report.end_time.format("%H:%M:%S")),
                report.total_pings,
                report.successful,
                report.failed,
                report.packet_loss_pct,
                report.loss_events,
                report.avg_latency_ms,
                report.min_latency_ms,
                report.max_latency_ms,
            )?;
        }
        writeln!(file)?;
    }

    // Loss timeline — show when drops happened
    let loss_periods = find_loss_periods(state);
    if !loss_periods.is_empty() {
        writeln!(file, "--- LOSS TIMELINE ---")?;
        writeln!(file)?;
        for (start_time, end_time, count) in &loss_periods {
            writeln!(file, "  {} → {}  ({} packets lost)",
                start_time.format("%H:%M:%S"),
                end_time.format("%H:%M:%S"),
                count,
            )?;
        }
        writeln!(file)?;
    }

    writeln!(file, "============================================================")?;
    writeln!(file, "  Generated by Network Monitor")?;
    writeln!(file, "============================================================")?;
    Ok(())
}

/// Write the raw console log output to a file
pub fn write_console_log(path: &std::path::Path, state: &PingState) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    writeln!(file, "Ping log — {} — target: {}", 
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        state.config.target)?;
    writeln!(file)?;
    for entry in &state.log_entries {
        writeln!(file, "{}", entry.message)?;
    }
    Ok(())
}

/// Find contiguous periods of packet loss with start/end times and packet counts
fn find_loss_periods(state: &PingState) -> Vec<(chrono::NaiveDateTime, chrono::NaiveDateTime, u64)> {
    let mut periods = Vec::new();
    let mut in_loss = false;
    let mut start = chrono::NaiveDateTime::default();
    let mut end = chrono::NaiveDateTime::default();
    let mut count: u64 = 0;

    for result in &state.results {
        if !result.success {
            if !in_loss {
                start = result.timestamp;
                count = 0;
                in_loss = true;
            }
            end = result.timestamp;
            count += 1;
        } else if in_loss {
            periods.push((start, end, count));
            in_loss = false;
        }
    }
    if in_loss {
        periods.push((start, end, count));
    }
    periods
}

// --- CSV export via csv crate + serde ---

#[derive(Serialize)]
struct CsvPingRow {
    seq: u64,
    success: bool,
    latency_ms: String,
    jitter_ms: String,
    timestamp: String,
    elapsed_secs: String,
}

#[derive(Serialize)]
struct CsvIntervalRow {
    start: String,
    end: String,
    total: u64,
    ok: u64,
    fail: u64,
    loss_pct: String,
    loss_events: u64,
    avg_ms: String,
    min_ms: String,
    max_ms: String,
}

/// Write ping results and interval reports to a CSV file via csv crate
pub fn write_csv(path: &std::path::Path, state: &PingState) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut writer = csv::Writer::from_writer(file);

    let mut prev_latency: Option<f64> = None;
    for result in &state.results {
        let jitter = match (result.latency_ms, prev_latency) {
            (Some(curr), Some(prev)) => format!("{:.1}", (curr - prev).abs()),
            _ => String::new(),
        };
        if result.latency_ms.is_some() {
            prev_latency = result.latency_ms;
        }

        writer.serialize(CsvPingRow {
            seq: result.seq,
            success: result.success,
            latency_ms: result.latency_ms.map(|lat| format!("{:.1}", lat)).unwrap_or_default(),
            jitter_ms: jitter,
            timestamp: result.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
            elapsed_secs: format!("{:.1}", result.elapsed_secs),
        }).map_err(csv_to_io_error)?;
    }

    writer.flush()?;
    // Write interval reports as a second section in the same file
    let mut file = writer.into_inner().map_err(|err| std::io::Error::other(err.to_string()))?;
    writeln!(file)?;
    writeln!(file, "--- Interval Reports ---")?;

    let mut report_writer = csv::Writer::from_writer(file);
    for report in &state.interval_reports {
        report_writer.serialize(CsvIntervalRow {
            start: report.start_time.format("%H:%M:%S").to_string(),
            end: report.end_time.format("%H:%M:%S").to_string(),
            total: report.total_pings,
            ok: report.successful,
            fail: report.failed,
            loss_pct: format!("{:.1}", report.packet_loss_pct),
            loss_events: report.loss_events,
            avg_ms: format!("{:.1}", report.avg_latency_ms),
            min_ms: format!("{:.1}", report.min_latency_ms),
            max_ms: format!("{:.1}", report.max_latency_ms),
        }).map_err(csv_to_io_error)?;
    }
    report_writer.flush()?;
    Ok(())
}

fn csv_to_io_error(err: csv::Error) -> std::io::Error {
    std::io::Error::other(err)
}

// --- JSON export via serde ---

#[derive(Serialize)]
struct JsonExport {
    test_info: JsonTestInfo,
    summary: JsonSummary,
    results: Vec<JsonResult>,
    interval_reports: Vec<JsonIntervalReport>,
}

#[derive(Serialize)]
struct JsonTestInfo {
    generated: String,
    target: String,
    duration: String,
    ping_interval_ms: u64,
    ping_timeout_ms: u32,
}

#[derive(Serialize)]
struct JsonSummary {
    total_sent: u64,
    total_received: u64,
    packets_lost: u64,
    packet_loss_pct: f64,
    loss_events: u64,
    avg_latency_ms: f64,
    min_latency_ms: f64,
    max_latency_ms: f64,
    avg_jitter_ms: f64,
}

#[derive(Serialize)]
struct JsonResult {
    seq: u64,
    success: bool,
    latency_ms: Option<f64>,
    timestamp: String,
    elapsed_secs: f64,
}

#[derive(Serialize)]
struct JsonIntervalReport {
    start: String,
    end: String,
    total: u64,
    ok: u64,
    fail: u64,
    loss_pct: f64,
    loss_events: u64,
    avg_ms: f64,
    min_ms: f64,
    max_ms: f64,
}

/// Write ping results and interval reports to a JSON file via serde
pub fn write_json(path: &std::path::Path, state: &PingState) -> std::io::Result<()> {
    let min = state.min_latency();
    let export = JsonExport {
        test_info: JsonTestInfo {
            generated: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            target: state.config.target.clone(),
            duration: state.elapsed_display(),
            ping_interval_ms: state.config.ping_interval_ms,
            ping_timeout_ms: state.config.timeout_ms,
        },
        summary: JsonSummary {
            total_sent: state.total_sent,
            total_received: state.total_received,
            packets_lost: state.total_sent - state.total_received,
            packet_loss_pct: round1(state.packet_loss_pct()),
            loss_events: state.loss_tracker.count,
            avg_latency_ms: round1(state.avg_latency()),
            min_latency_ms: round1(if min == f64::MAX { 0.0 } else { min }),
            max_latency_ms: round1(state.max_latency()),
            avg_jitter_ms: round1(state.jitter.avg()),
        },
        results: state.results.iter().map(|result| JsonResult {
            seq: result.seq,
            success: result.success,
            latency_ms: result.latency_ms.map(round1),
            timestamp: result.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
            elapsed_secs: round1(result.elapsed_secs),
        }).collect(),
        interval_reports: state.interval_reports.iter().map(|report| JsonIntervalReport {
            start: report.start_time.format("%H:%M:%S").to_string(),
            end: report.end_time.format("%H:%M:%S").to_string(),
            total: report.total_pings,
            ok: report.successful,
            fail: report.failed,
            loss_pct: round1(report.packet_loss_pct),
            loss_events: report.loss_events,
            avg_ms: round1(report.avg_latency_ms),
            min_ms: round1(report.min_latency_ms),
            max_ms: round1(report.max_latency_ms),
        }).collect(),
    };

    let file = std::fs::File::create(path)?;
    serde_json::to_writer_pretty(file, &export)
        .map_err(std::io::Error::other)
}

/// Round to 1 decimal place for clean JSON output
fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}
