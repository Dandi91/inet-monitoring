use crossbeam_channel::{Receiver, RecvTimeoutError};
use heck::ToSnakeCase;
use lazy_static::lazy_static;
use prometheus::{HistogramVec, IntCounterVec, register_histogram_vec, register_int_counter_vec};
use std::process::Command;
use std::str;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

lazy_static! {
    static ref PING_DELAY: HistogramVec =
        register_histogram_vec!("ping_delay_seconds", "ping delay in seconds", &["hostname"]).unwrap();
    static ref PING_FAILS: IntCounterVec =
        register_int_counter_vec!("ping_fails", "number of failed pings", &["hostname", "error_type"]).unwrap();
    
    // Regex for "time=12.3 ms" or "time<1 ms"
    static ref TIME_PATTERN: regex::Regex = 
        regex::Regex::new(r"(?i)time[=<](\d+\.?\d*)\s*ms").unwrap();
    
    // Regex for "rtt min/avg/max/mdev = a/b/c/d ms" or "round-trip min/avg/max/stddev = a/b/c/d ms"
    static ref RTT_PATTERN: regex::Regex = 
        regex::Regex::new(r"(?i)(?:rtt|round[- ]?trip)\s+[^=]+=\s*([\d.]+)/([\d.]+)/([\d.]+)/([\d.]+)\s*ms").unwrap();
}

pub fn run(targets: Vec<String>, delay: Duration, timeout: Duration, shutdown_rx: Receiver<()>) -> JoinHandle<()> {
    thread::spawn(move || {
        loop {
            for target in &targets {
                match ping_target(target, timeout) {
                    Ok(latency) => PING_DELAY.with_label_values(&[target]).observe(latency.as_secs_f64()),
                    Err(err) => {
                        eprintln!("failed to ping {}: {}", target, err);
                        PING_FAILS.with_label_values(&[target, &err.to_snake_case()]).inc();
                    }
                }
            }

            match shutdown_rx.recv_timeout(delay) {
                Ok(_) | Err(RecvTimeoutError::Disconnected) => {
                    println!("ping thread shutting down");
                    return;
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    })
}

fn ping_args(target: &str, timeout: Duration) -> [&str; 5] {
    // Build platform-specific args:
    // - Linux/BusyBox: ping -c 1 -W <timeout_seconds>
    // - macOS/BSD:     ping -c 1 -W <timeout_ms> (BSD uses milliseconds)
    #[cfg(target_os = "macos")]
    let timeout_arg = timeout.as_millis().max(1).to_string(); // at least 1 ms
    #[cfg(not(target_os = "macos"))]
    let timeout_arg = timeout.as_secs().max(1).to_string(); // at least 1 s for Linux/BusyBox
   ["-c", "1", "-W", &timeout_arg, target]
}

/// Execute system `ping` and parse output for latency.
/// Supports common Linux/macOS ping formats.
/// Returns Err(...) with a short snake_case string describing the failure cause.
fn ping_target(target: &str, timeout: Duration) -> Result<Duration, String> {
    let args = ping_args(target, timeout);

    let output = Command::new("ping")
        .args(args)
        .output()
        .map_err(|e| format!("io_error_{}", e.kind().to_string().to_snake_case()))?;

    if !output.status.success() {
        // Non-zero exit usually means timeout or unreachable.
        // Try to detect timeout keywords from stderr/stdout.
        let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
        let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
        let msg = if stderr.contains("timed out") || stdout.contains("timed out") || stdout.contains("timeout") {
            "timeout"
        } else if stderr.contains("unknown host") || stdout.contains("unknown host") {
            "unknown_host"
        } else if stderr.contains("permission") {
            "permission_denied"
        } else {
            "nonzero_exit"
        };
        return Err(msg.to_string());
    }

    // Parse latency from stdout. Common patterns:
    // - time=12.3 ms
    // - time<1 ms
    // - rtt min/avg/max/mdev = 11.234/11.234/11.234/0.000 ms
    let out = String::from_utf8_lossy(&output.stdout);

    // First try time=... pattern
    if let Some(dur) = parse_time_ms_from_line(&out) {
        println!("pinging {} took {:?}", target, dur);
        return Ok(dur);
    }

    // Fallback: parse avg from rtt line
    if let Some(dur) = parse_rtt_avg_ms(&out) {
        println!("pinging {} took {:?}", target, dur);
        return Ok(dur);
    }

    Err("parse_error".to_string())
}

// Extract "time=12.3 ms" or "time<1 ms" patterns
fn parse_time_ms_from_line(s: &str) -> Option<Duration> {
    TIME_PATTERN.captures(s).and_then(|caps| {
        caps.get(1)?.as_str().parse::<f64>().ok().map(|ms| {
            Duration::from_secs_f64(ms / 1000.0)
        })
    })
}

// Extract avg from "rtt min/avg/max/mdev = a/b/c/d ms"
fn parse_rtt_avg_ms(s: &str) -> Option<Duration> {
    RTT_PATTERN.captures(s).and_then(|caps| {
        caps.get(2)?.as_str().parse::<f64>().ok().map(|avg_ms| {
            Duration::from_secs_f64(avg_ms / 1000.0)
        })
    })
}
