//! # Background Pinger Thread
//!
//! Spawns dedicated threads that continuously ping the configured target host
//! and optionally the default gateway. Results are pushed into shared state
//! for the GUI to read.
//!
//! On Windows, all `ping.exe` subprocesses are spawned with the `CREATE_NO_WINDOW`
//! creation flag to prevent console popups from appearing in the background.

use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::core::state::{IntervalReport, PingResult, SharedState};

/// Windows process creation flag that prevents a console window from being created.
/// Without this, every `ping.exe` invocation would flash a CMD window.
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Spawn the background pinger thread for the external target.
pub fn start_pinger(state: SharedState) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        pinger_loop(state);
    })
}

/// Spawn a separate background thread that pings the gateway at the same frequency.
/// Only sends pings when both `running` and `gateway_enabled` are true.
pub fn start_gateway_pinger(state: SharedState) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        gateway_pinger_loop(state);
    })
}

/// Detect the default gateway IP by parsing `ipconfig` output on Windows.
/// Returns None if no gateway is found or on non-Windows platforms.
pub fn detect_gateway() -> Option<String> {
    let mut cmd = Command::new("ipconfig");

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                // Look for "Default Gateway" lines with an actual IP
                if line.contains("Default Gateway") {
                    if let Some(colon_pos) = line.rfind(':') {
                        let ip = line[colon_pos + 1..].trim();
                        // Validate it looks like an IPv4 address
                        if !ip.is_empty() && ip.contains('.') && ip != "0.0.0.0" {
                            return Some(ip.to_string());
                        }
                    }
                }
            }
            None
        }
        Err(_) => None,
    }
}

/// Main pinger loop. Runs indefinitely, checking the `running` flag each iteration.
fn pinger_loop(state: SharedState) {
    loop {
        let config_snapshot = read_config_snapshot(&state);

        if !config_snapshot.running {
            thread::sleep(Duration::from_millis(200));
            continue;
        }

        if check_and_stop_if_duration_exceeded(&state, config_snapshot.duration_secs) {
            continue;
        }

        let ping_start = Instant::now();
        let (success, latency_ms, output_line) = execute_ping(
            &config_snapshot.target, config_snapshot.timeout_ms
        );

        record_ping_result(
            &state, success, latency_ms, &output_line,
            &config_snapshot.target, config_snapshot.interval_secs,
        );

        sleep_until_next_ping(ping_start, config_snapshot.ping_interval_ms);
    }
}

/// Snapshot of config values read under a single lock
struct ConfigSnapshot {
    target: String,
    timeout_ms: u32,
    interval_secs: u64,
    ping_interval_ms: u64,
    duration_secs: u64,
    running: bool,
}

fn read_config_snapshot(state: &SharedState) -> ConfigSnapshot {
    let shared = state.lock().unwrap_or_else(|err| err.into_inner());
    ConfigSnapshot {
        target: shared.config.target.clone(),
        timeout_ms: shared.config.timeout_ms,
        interval_secs: shared.config.interval_secs,
        ping_interval_ms: shared.config.ping_interval_ms,
        duration_secs: shared.config.duration_secs,
        running: shared.running,
    }
}

/// Check duration and auto-stop in a single lock (avoids race condition).
/// Returns true if the test was stopped.
fn check_and_stop_if_duration_exceeded(state: &SharedState, duration_secs: u64) -> bool {
    if duration_secs == 0 {
        return false;
    }
    let mut shared = state.lock().unwrap_or_else(|err| err.into_inner());
    if shared.elapsed_secs() >= duration_secs as f64 {
        shared.flush_partial_report();
        shared.running = false;
        shared.push_log(format!(
            "[{}] ⏱ Test duration reached — stopped automatically",
            chrono::Local::now().naive_local().format("%H:%M:%S")
        ));
        true
    } else {
        false
    }
}

/// Record a ping result into shared state under a single lock.
fn record_ping_result(
    state: &SharedState,
    success: bool,
    latency_ms: Option<f64>,
    output_line: &str,
    target: &str,
    interval_secs: u64,
) {
    let now = chrono::Local::now().naive_local();
    let mut shared = state.lock().unwrap_or_else(|err| err.into_inner());

    if shared.config_changed {
        shared.interval_start = None;
        shared.interval_start_time = None;
        shared.interval_results.clear();
        shared.config_changed = false;
    }

    if shared.start_time.is_none() {
        shared.start_time = Some(Instant::now());
    }
    let elapsed_secs = shared.start_time
        .map(|start| start.elapsed().as_secs_f64())
        .unwrap_or(0.0);

    shared.seq_counter += 1;
    let seq = shared.seq_counter;
    shared.total_sent += 1;
    if success {
        shared.total_received += 1;
    }

    let result = PingResult {
        seq,
        success,
        latency_ms,
        timestamp: now,
        elapsed_secs,
    };
    shared.push_result(result.clone());

    let log_msg = if success {
        format!(
            "[{}] #{} Reply from {}: time={}ms",
            now.format("%H:%M:%S"), seq, target,
            latency_ms.map(|lat| format!("{:.0}", lat)).unwrap_or("?".into())
        )
    } else {
        format!("[{}] #{} Request timed out ({})", now.format("%H:%M:%S"), seq, output_line)
    };
    shared.push_log(log_msg);

    accumulate_interval(&mut shared, result, now, interval_secs);
}

/// Accumulate results for the current interval and generate a report when elapsed.
fn accumulate_interval(
    shared: &mut crate::core::state::PingState,
    result: PingResult,
    now: chrono::NaiveDateTime,
    interval_secs: u64,
) {
    if shared.interval_start.is_none() {
        shared.interval_start = Some(Instant::now());
        shared.interval_start_time = Some(now);
    }
    shared.interval_results.push(result);

    if let Some(start) = shared.interval_start {
        if start.elapsed() >= Duration::from_secs(interval_secs) {
            let report = generate_report(
                &shared.interval_results,
                shared.interval_start_time.unwrap_or(now),
                now,
            );
            shared.interval_reports.push_back(report);
            if shared.interval_reports.len() > 256 {
                shared.interval_reports.pop_front();
            }
            shared.interval_results.clear();
            shared.interval_start = Some(Instant::now());
            shared.interval_start_time = Some(now);
        }
    }
}

/// Sleep to maintain the configured ping cadence, minus time already spent.
fn sleep_until_next_ping(ping_start: Instant, ping_interval_ms: u64) {
    let interval = Duration::from_millis(ping_interval_ms);
    let elapsed = ping_start.elapsed();
    if elapsed < interval {
        thread::sleep(interval - elapsed);
    }
}

/// Gateway pinger loop. Pings the gateway IP at the same frequency as the
/// external target. Only active when gateway_enabled is true.
fn gateway_pinger_loop(state: SharedState) {
    loop {
        let (gateway_ip, timeout_ms, ping_interval_ms, running, enabled) = {
            let shared = state.lock().unwrap_or_else(|err| err.into_inner());
            (
                shared.gateway_ip.clone(),
                shared.config.timeout_ms,
                shared.config.ping_interval_ms,
                shared.running,
                shared.gateway_enabled,
            )
        };

        if !running || !enabled || gateway_ip.is_none() {
            thread::sleep(Duration::from_millis(200));
            continue;
        }

        let gw_ip = gateway_ip.unwrap();
        let ping_start = Instant::now();
        let (success, latency_ms, _) = execute_ping(&gw_ip, timeout_ms);

        {
            let mut shared = state.lock().unwrap_or_else(|err| err.into_inner());
            shared.push_gateway_result(latency_ms, success);
        }

        let interval = Duration::from_millis(ping_interval_ms);
        let elapsed = ping_start.elapsed();
        if elapsed < interval {
            thread::sleep(interval - elapsed);
        }
    }
}

/// Execute a single ping via the system `ping` command.
///
/// Returns a tuple of (success, latency_ms, summary_line):
/// - `success`: true if a reply was received
/// - `latency_ms`: parsed round-trip time, or None on timeout
/// - `summary_line`: the most relevant stdout line for logging
pub fn execute_ping(target: &str, timeout_ms: u32) -> (bool, Option<f64>, String) {
    let mut cmd = Command::new("ping");
    // -n 1: send exactly one ICMP echo request
    // -w <timeout>: wait at most this many milliseconds for a reply
    cmd.args(["-n", "1", "-w", &timeout_ms.to_string(), target]);

    // Prevent a visible console window from flashing on each ping
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let success = output.status.success() && stdout.contains("time=") || stdout.contains("time<");

            let latency = parse_latency(&stdout);
            let summary = stdout
                .lines()
                .find(|l| l.contains("time=") || l.contains("time<") || l.contains("timed out") || l.contains("unreachable"))
                .unwrap_or("no response")
                .trim()
                .to_string();

            (success, latency, summary)
        }
        Err(e) => (false, None, format!("Failed to execute ping: {}", e)),
    }
}

/// Parse the round-trip latency from ping's stdout output.
///
/// Handles two Windows ping output formats:
/// - `time=15ms` (normal response)
/// - `time<1ms` (sub-millisecond response)
fn parse_latency(output: &str) -> Option<f64> {
    for line in output.lines() {
        if let Some(pos) = line.find("time=") {
            let after = &line[pos + 5..];
            let num_str: String = after.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            if let Ok(val) = num_str.parse::<f64>() {
                return Some(val);
            }
        }
        if let Some(pos) = line.find("time<") {
            let after = &line[pos + 5..];
            let num_str: String = after.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            if let Ok(val) = num_str.parse::<f64>() {
                return Some(val);
            }
        }
    }
    None
}

/// Generate a summary report for a completed interval.
fn generate_report(
    results: &[PingResult],
    start_time: chrono::NaiveDateTime,
    end_time: chrono::NaiveDateTime,
) -> IntervalReport {
    let total = results.len() as u64;
    let successful = results.iter().filter(|result| result.success).count() as u64;
    let failed = total - successful;
    let packet_loss_pct = if total > 0 {
        (failed as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let latencies: Vec<f64> = results.iter().filter_map(|result| result.latency_ms).collect();
    let avg = if latencies.is_empty() {
        0.0
    } else {
        latencies.iter().sum::<f64>() / latencies.len() as f64
    };
    let min = latencies.iter().cloned().fold(f64::MAX, f64::min);
    let max = latencies.iter().cloned().fold(0.0_f64, f64::max);

    IntervalReport {
        start_time,
        end_time,
        total_pings: total,
        successful,
        failed,
        packet_loss_pct,
        avg_latency_ms: avg,
        min_latency_ms: if min == f64::MAX { 0.0 } else { min },
        max_latency_ms: max,
        loss_events: crate::core::state::count_loss_events(results),
    }
}
