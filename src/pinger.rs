//! # Background Pinger Thread
//!
//! Spawns a dedicated thread that continuously pings the configured target host
//! at ~1-second intervals. Results are pushed into shared state for the GUI to read.
//!
//! On Windows, all `ping.exe` subprocesses are spawned with the `CREATE_NO_WINDOW`
//! creation flag to prevent console popups from appearing in the background.

use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::state::{IntervalReport, PingResult, SharedState};

/// Windows process creation flag that prevents a console window from being created.
/// Without this, every `ping.exe` invocation would flash a CMD window.
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Spawn the background pinger thread. Returns a JoinHandle to keep it alive.
pub fn start_pinger(state: SharedState) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        pinger_loop(state);
    })
}

/// Main pinger loop. Runs indefinitely, checking the `running` flag each iteration.
/// When stopped, it polls every 200ms to be responsive to start commands.
fn pinger_loop(state: SharedState) {
    loop {
        // Read config snapshot outside the write lock to minimize lock contention.
        // We clone the target string once per iteration rather than per-use.
        let (target, timeout_ms, interval_secs, running) = {
            let s = state.read().unwrap_or_else(|e| e.into_inner());
            (
                s.config.target.clone(),
                s.config.timeout_ms,
                s.config.interval_secs,
                s.running,
            )
        };

        // If monitoring is paused, sleep briefly and re-check
        if !running {
            thread::sleep(Duration::from_millis(200));
            continue;
        }

        // Execute the ping and measure wall-clock time for sleep compensation
        let ping_start = Instant::now();
        let (success, latency_ms, output_line) = execute_ping(&target, timeout_ms);
        let now = chrono::Local::now().naive_local();

        // Acquire write lock to update shared state with this ping's results
        {
            let mut s = state.write().unwrap_or_else(|e| e.into_inner());

            // If the user changed config via the GUI, reset the current interval
            if s.config_changed {
                s.interval_start = None;
                s.interval_start_time = None;
                s.interval_results.clear();
                s.config_changed = false;
            }

            // Update counters
            s.seq_counter += 1;
            let seq = s.seq_counter;
            s.total_sent += 1;
            if success {
                s.total_received += 1;
            }

            let result = PingResult {
                seq,
                success,
                latency_ms,
                timestamp: now,
            };

            // Push result into the bounded ring buffer
            s.push_result(result.clone());

            // Format a human-readable log message for the console tab
            let log_msg = if success {
                format!(
                    "[{}] #{} Reply from {}: time={}ms",
                    now.format("%H:%M:%S"),
                    seq,
                    target,
                    latency_ms.map(|l| format!("{:.0}", l)).unwrap_or("?".into())
                )
            } else {
                format!(
                    "[{}] #{} Request timed out ({})",
                    now.format("%H:%M:%S"),
                    seq,
                    output_line
                )
            };
            s.push_log(log_msg);

            // --- Interval report accumulation ---
            // Start a new interval if none is active
            if s.interval_start.is_none() {
                s.interval_start = Some(Instant::now());
                s.interval_start_time = Some(now);
            }
            s.interval_results.push(result);

            // Check if the current interval has elapsed; if so, generate a report
            if let Some(start) = s.interval_start {
                if start.elapsed() >= Duration::from_secs(interval_secs) {
                    let report = generate_report(
                        &s.interval_results,
                        s.interval_start_time.unwrap_or(now),
                        now,
                    );
                    s.interval_reports.push_back(report);
                    // Cap interval reports at 256 entries
                    if s.interval_reports.len() > 256 {
                        s.interval_reports.pop_front();
                    }
                    // Reset for the next interval
                    s.interval_results.clear();
                    s.interval_start = Some(Instant::now());
                    s.interval_start_time = Some(now);
                }
            }
        }

        // Compensate sleep to maintain ~1 ping/second cadence.
        // Subtracts the time already spent executing the ping.
        let elapsed = ping_start.elapsed();
        if elapsed < Duration::from_secs(1) {
            thread::sleep(Duration::from_secs(1) - elapsed);
        }
    }
}

/// Execute a single ping via the system `ping` command.
///
/// Returns a tuple of (success, latency_ms, summary_line):
/// - `success`: true if a reply was received
/// - `latency_ms`: parsed round-trip time, or None on timeout
/// - `summary_line`: the most relevant stdout line for logging
fn execute_ping(target: &str, timeout_ms: u32) -> (bool, Option<f64>, String) {
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
            // A ping is successful if the process exited OK and output contains latency info.
            // "time<1ms" appears on Windows for sub-millisecond responses.
            let success = output.status.success() && stdout.contains("time=") || stdout.contains("time<");

            let latency = parse_latency(&stdout);
            // Extract the most informative line from stdout for the log
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
        // Try "time=Xms" format first
        if let Some(pos) = line.find("time=") {
            let after = &line[pos + 5..];
            let num_str: String = after.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            if let Ok(val) = num_str.parse::<f64>() {
                return Some(val);
            }
        }
        // Fall back to "time<Xms" format (sub-millisecond replies)
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
///
/// Aggregates all ping results within the interval into statistics:
/// total/successful/failed counts, packet loss %, and latency min/avg/max.
fn generate_report(
    results: &[PingResult],
    start_time: chrono::NaiveDateTime,
    end_time: chrono::NaiveDateTime,
) -> IntervalReport {
    let total = results.len() as u64;
    let successful = results.iter().filter(|r| r.success).count() as u64;
    let failed = total - successful;
    let packet_loss_pct = if total > 0 {
        (failed as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    // Collect only successful latencies for statistical calculations
    let latencies: Vec<f64> = results.iter().filter_map(|r| r.latency_ms).collect();
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
    }
}
