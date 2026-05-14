//! # Server Check
//!
//! On-demand connectivity tests for multiple targets using either ICMP ping
//! or TCP connect. Used by the Servers tab for quick-check and by the main
//! pinger when the selected preset uses TCP mode.

use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::core::config::TargetPreset;

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// How a target should be tested
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TestMode {
    Icmp,
    Tcp { port: u16 },
}

impl Default for TestMode {
    fn default() -> Self {
        TestMode::Icmp
    }
}

impl TestMode {
    pub fn label(&self) -> String {
        match self {
            TestMode::Icmp => "ICMP".into(),
            TestMode::Tcp { port } => format!("TCP:{}", port),
        }
    }

    pub fn is_tcp(&self) -> bool {
        matches!(self, TestMode::Tcp { .. })
    }

    pub fn port(&self) -> u16 {
        match self {
            TestMode::Tcp { port } => *port,
            TestMode::Icmp => 443,
        }
    }
}

/// Outcome of a single server check
#[derive(Clone, Debug)]
pub enum CheckStatus {
    Ok,
    Timeout,
    Error(String),
}

/// Result of checking one server
#[derive(Clone, Debug)]
pub struct ServerCheckResult {
    pub name: String,
    pub host: String,
    pub mode: TestMode,
    pub category: String,
    pub status: CheckStatus,
    pub latency_ms: Option<f64>,
    pub checked_at: chrono::NaiveDateTime,
}

/// Perform a TCP connect test. Measures the time for the TCP handshake.
pub fn check_tcp(host: &str, port: u16, timeout_ms: u32) -> (CheckStatus, Option<f64>) {
    let addr_str = format!("{}:{}", host, port);
    let timeout = Duration::from_millis(timeout_ms as u64);

    let addr = match addr_str.to_socket_addrs() {
        Ok(mut addrs) => match addrs.next() {
            Some(a) => a,
            None => return (CheckStatus::Error("DNS resolution failed".into()), None),
        },
        Err(e) => return (CheckStatus::Error(format!("DNS error: {}", e)), None),
    };

    let start = Instant::now();
    match TcpStream::connect_timeout(&addr, timeout) {
        Ok(_stream) => {
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            (CheckStatus::Ok, Some(latency))
        }
        Err(e) => {
            if start.elapsed() >= timeout {
                (CheckStatus::Timeout, None)
            } else {
                (CheckStatus::Error(format!("{}", e)), None)
            }
        }
    }
}

/// Perform an ICMP ping test using the system `ping` command.
pub fn check_icmp(host: &str, timeout_ms: u32) -> (CheckStatus, Option<f64>) {
    let mut cmd = Command::new("ping");
    cmd.args(["-n", "1", "-w", &timeout_ms.to_string(), host]);

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let success = (output.status.success() && stdout.contains("time=")) || stdout.contains("time<");

            if success {
                let latency = parse_latency(&stdout);
                (CheckStatus::Ok, latency)
            } else {
                (CheckStatus::Timeout, None)
            }
        }
        Err(e) => (CheckStatus::Error(format!("Failed to execute ping: {}", e)), None),
    }
}

/// Parse "time=Xms" or "time<1ms" from ping output
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

/// Check a single server based on its preset configuration
pub fn check_server(preset: &TargetPreset, timeout_ms: u32) -> ServerCheckResult {
    let (status, latency_ms) = match &preset.mode {
        TestMode::Icmp => check_icmp(&preset.host, timeout_ms),
        TestMode::Tcp { port } => check_tcp(&preset.host, *port, timeout_ms),
    };

    ServerCheckResult {
        name: preset.name.clone(),
        host: preset.host.clone(),
        mode: preset.mode.clone(),
        category: preset.category.clone(),
        status,
        latency_ms,
        checked_at: chrono::Local::now().naive_local(),
    }
}
