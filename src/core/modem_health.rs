//! # Modem Health Check
//!
//! Periodically performs an HTTP GET to the modem's status page to detect
//! CPU/firmware issues. A dying modem may respond to ICMP pings fine but
//! fail to serve HTTP requests.
//!
//! Only supports plain HTTP (not HTTPS). Accepts any valid HTTP response
//! (200, 301, 401, etc.) as success - we just need proof the web server is alive.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::core::state::{SharedState, ModemHttpStatus, ShutdownSignal, lock_state};

/// Start the modem health check background thread.
/// Returns (join handle, shutdown signal).
pub fn start_modem_health_checker(state: SharedState) -> (std::thread::JoinHandle<()>, ShutdownSignal) {
    let shutdown = crate::core::state::new_shutdown_signal();
    let signal = shutdown.clone();
    let handle = std::thread::spawn(move || {
        health_check_loop(state, signal);
    });
    (handle, shutdown)
}

fn health_check_loop(state: SharedState, shutdown: ShutdownSignal) {
    while !shutdown.load(std::sync::atomic::Ordering::Relaxed) {
        {
            lock_state(&state).heartbeats.modem = Some(std::time::Instant::now());
        }
        let (enabled, url, interval_secs) = {
            let shared = lock_state(&state);
            (
                shared.modem.enabled,
                shared.modem.url.clone(),
                shared.modem.interval_secs,
            )
        };

        if !enabled || url.is_empty() {
            std::thread::sleep(Duration::from_secs(2));
            continue;
        }

        let result = check_http(&url);

        {
            let mut shared = lock_state(&state);
            shared.modem.http_status = result;
        }

        std::thread::sleep(Duration::from_secs(interval_secs as u64));
    }
}

/// Perform a minimal HTTP GET and check for a valid response.
fn check_http(url: &str) -> ModemHttpStatus {
    let Some((host, port, path)) = parse_http_url(url) else {
        return ModemHttpStatus::Failed("Invalid URL".into());
    };

    let addr = format!("{}:{}", host, port);
    let timeout = Duration::from_secs(3);

    // TCP connect
    let mut stream = match TcpStream::connect_timeout(
        &match addr.parse() {
            Ok(a) => a,
            Err(_) => {
                // Try resolving as hostname:port
                use std::net::ToSocketAddrs;
                match addr.to_socket_addrs().and_then(|mut i| i.next().ok_or(
                    std::io::Error::new(std::io::ErrorKind::Other, "No address found")
                )) {
                    Ok(a) => a,
                    Err(e) => return ModemHttpStatus::Failed(format!("DNS: {}", e)),
                }
            }
        },
        timeout,
    ) {
        Ok(s) => s,
        Err(e) => return ModemHttpStatus::Failed(format!("Connect: {}", e)),
    };

    stream.set_read_timeout(Some(timeout)).ok();
    stream.set_write_timeout(Some(timeout)).ok();

    // Send minimal HTTP GET
    let request = format!(
        "GET {} HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, host
    );
    if let Err(e) = stream.write_all(request.as_bytes()) {
        return ModemHttpStatus::Failed(format!("Write: {}", e));
    }

    // Read first line of response
    let mut buf = [0u8; 256];
    match stream.read(&mut buf) {
        Ok(0) => ModemHttpStatus::Failed("Empty response".into()),
        Ok(n) => {
            let response = String::from_utf8_lossy(&buf[..n]);
            if response.starts_with("HTTP/") {
                ModemHttpStatus::Ok
            } else {
                ModemHttpStatus::Failed("Not HTTP".into())
            }
        }
        Err(e) => ModemHttpStatus::Failed(format!("Read: {}", e)),
    }
}

/// Parse a simple HTTP URL into (host, port, path).
/// Only supports `http://host[:port]/path` format.
fn parse_http_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("http://")?;
    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match host_port.rfind(':') {
        Some(i) => (&host_port[..i], host_port[i + 1..].parse().ok()?),
        None => (host_port, 80u16),
    };
    Some((host.to_string(), port, path.to_string()))
}
