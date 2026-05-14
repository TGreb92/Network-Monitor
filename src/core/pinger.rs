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

use crate::core::server_check::TestMode;
use crate::core::state::{PingResult, SharedState, ShutdownSignal, lock_state};

/// Windows process creation flag that prevents a console window from being created.
/// Without this, every `ping.exe` invocation would flash a CMD window.
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Spawn the background pinger thread for the external target.
/// Returns (join handle, shutdown signal).
pub fn start_pinger(state: SharedState) -> (thread::JoinHandle<()>, ShutdownSignal) {
    let shutdown = crate::core::state::new_shutdown_signal();
    let signal = shutdown.clone();
    let handle = thread::spawn(move || {
        pinger_loop(state, signal);
    });
    (handle, shutdown)
}

/// Spawn a separate background thread that pings the gateway at the same frequency.
/// Returns (join handle, shutdown signal).
pub fn start_gateway_pinger(state: SharedState) -> (thread::JoinHandle<()>, ShutdownSignal) {
    let shutdown = crate::core::state::new_shutdown_signal();
    let signal = shutdown.clone();
    let handle = thread::spawn(move || {
        gateway_pinger_loop(state, signal);
    });
    (handle, shutdown)
}

/// Detect the default gateway IP by parsing `ipconfig` output on Windows.
/// Returns None if no gateway is found or on non-Windows platforms.
pub fn detect_gateway() -> Option<String> {
    let mut cmd = Command::new("ipconfig");

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        if !line.contains("Default Gateway") {
            continue;
        }
        let Some(colon_pos) = line.rfind(':') else { continue };
        let ip = line[colon_pos + 1..].trim();
        if !ip.is_empty() && ip.contains('.') && ip != "0.0.0.0" {
            return Some(ip.to_string());
        }
    }
    None
}

/// Main pinger loop. Runs indefinitely, checking the `running` flag each iteration.
fn pinger_loop(state: SharedState, shutdown: ShutdownSignal) {
    while !shutdown.load(std::sync::atomic::Ordering::Relaxed) {
        {
            lock_state(&state).heartbeats.pinger = Some(Instant::now());
        }
        // Quick check: only read running flag (no String clone)
        let running = {
            let shared = lock_state(&state);
            shared.running
        };

        if !running {
            thread::sleep(Duration::from_millis(200));
            continue;
        }

        // Full config read (clones target String) only when running
        let config_snapshot = read_config_snapshot(&state);

        if check_and_stop_if_duration_exceeded(&state, config_snapshot.duration_secs) {
            continue;
        }

        let ping_start = Instant::now();
        let (success, latency_ms, output_line) = match &config_snapshot.test_mode {
            TestMode::Tcp { port } => {
                let (status, lat) = crate::core::server_check::check_tcp(
                    &config_snapshot.target, *port, config_snapshot.timeout_ms
                );
                let success = matches!(status, crate::core::server_check::CheckStatus::Ok);
                let line = match status {
                    crate::core::server_check::CheckStatus::Ok =>
                        format!("TCP {}:{} time={:.0}ms", config_snapshot.target, port, lat.unwrap_or(0.0)),
                    crate::core::server_check::CheckStatus::Timeout => "TCP connect timed out".into(),
                    crate::core::server_check::CheckStatus::Error(e) => format!("TCP error: {}", e),
                };
                (success, lat, line)
            }
            TestMode::Icmp => {
                execute_ping(&config_snapshot.target, config_snapshot.timeout_ms)
            }
        };

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
    test_mode: TestMode,
}

fn read_config_snapshot(state: &SharedState) -> ConfigSnapshot {
    let shared = lock_state(&state);
    ConfigSnapshot {
        target: shared.config.target.clone(),
        timeout_ms: shared.config.timeout_ms,
        interval_secs: shared.config.interval_secs,
        ping_interval_ms: shared.config.ping_interval_ms,
        duration_secs: shared.config.duration_secs,
        test_mode: shared.config.test_mode.clone(),
    }
}

/// Check duration and auto-stop in a single lock (avoids race condition).
/// Returns true if the test was stopped.
fn check_and_stop_if_duration_exceeded(state: &SharedState, duration_secs: u64) -> bool {
    if duration_secs == 0 {
        return false;
    }
    let mut shared = lock_state(&state);
    if shared.elapsed_secs() >= duration_secs as f64 {
        shared.stop();
        shared.auto_export_pending = true;
        shared.push_log(format!(
            "[{}] ⏱ Test duration reached - stopped automatically",
            chrono::Local::now().naive_local().format("%H:%M:%S")
        ), None, true);
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
    let mut shared = lock_state(&state);

    if shared.config_changed {
        shared.interval.start = None;
        shared.interval.start_time = None;
        shared.interval.results.clear();
        shared.config_changed = false;
    }

    let start = shared.start_time.get_or_insert_with(Instant::now);
    let elapsed_secs = start.elapsed().as_secs_f64();

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
    shared.push_log(log_msg, latency_ms, success);

    accumulate_interval(&mut shared, result, now, interval_secs);
}

/// Accumulate results for the current interval and generate a report when elapsed.
fn accumulate_interval(
    shared: &mut crate::core::state::PingState,
    result: PingResult,
    now: chrono::NaiveDateTime,
    interval_secs: u64,
) {
    if shared.interval.start.is_none() {
        shared.interval.start = Some(Instant::now());
        shared.interval.start_time = Some(now);
    }
    shared.interval.results.push(result);

    let Some(start) = shared.interval.start else { return };
    if start.elapsed() < Duration::from_secs(interval_secs) {
        return;
    }

    let report = crate::core::state::build_interval_report(
        &shared.interval.results,
        shared.interval.start_time.unwrap_or(now),
        now,
    );
    shared.interval_reports.push_back(report);
    if shared.interval_reports.len() > 256 {
        shared.interval_reports.pop_front();
    }
    shared.interval.results.clear();
    shared.interval.start = Some(Instant::now());
    shared.interval.start_time = Some(now);
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
fn gateway_pinger_loop(state: SharedState, shutdown: ShutdownSignal) {
    while !shutdown.load(std::sync::atomic::Ordering::Relaxed) {
        {
            lock_state(&state).heartbeats.gateway = Some(Instant::now());
        }
        let (gateway_ip, timeout_ms, ping_interval_ms, running, enabled) = {
            let shared = lock_state(&state);
            (
                shared.gateway.ip.clone(),
                shared.config.timeout_ms,
                shared.config.ping_interval_ms,
                shared.running,
                shared.gateway.enabled,
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
            let mut shared = lock_state(&state);
            shared.gateway.push_result(latency_ms, success);
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
        let pos = line.find("time=").or_else(|| line.find("time<"));
        if let Some(pos) = pos {
            let after = &line[pos + 5..];
            let num_str: String = after.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            return num_str.parse().ok();
        }
    }
    None
}
