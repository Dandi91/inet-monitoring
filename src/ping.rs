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

/// Execute system `ping` and parse output for latency.
/// Supports common Linux/macOS ping formats.
/// Returns Err(...) with a short snake_case string describing the failure cause.
fn ping_target(target: &str, timeout: Duration) -> Result<Duration, String> {
    // Build platform-specific args:
    // - Linux/BusyBox: ping -c 1 -W <timeout_seconds>
    // - macOS/BSD:     ping -c 1 -W <timeout_ms> (BSD uses milliseconds)
    #[cfg(target_os = "macos")]
    let timeout_arg = (timeout.as_millis().max(1)).to_string(); // at least 1 ms
    #[cfg(not(target_os = "macos"))]
    let timeout_arg = (timeout.as_secs().max(1)).to_string(); // at least 1 s for Linux/BusyBox

    #[cfg(target_os = "macos")]
    let args = ["-c", "1", "-W", &timeout_arg, target];

    #[cfg(not(target_os = "macos"))]
    let args = ["-c", "1", "-W", &timeout_arg, target];

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
    // Find "time=" occurrence
    for line in s.lines() {
        let lower = line.to_lowercase();
        if let Some(idx) = lower.find("time=") {
            let rest = &lower[idx + 5..];
            // Accept forms like "12.3 ms", "0.8 ms", "<1 ms"
            // Strip leading '<' if present
            let rest = rest.trim_start();
            let rest = rest.strip_prefix('<').unwrap_or(rest);
            // Take until " ms"
            if let Some(ms_idx) = rest.find(" ms") {
                let num = &rest[..ms_idx].trim();
                if let Ok(v) = num.parse::<f64>() {
                    // value given is in milliseconds
                    let secs = v / 1000.0;
                    return Some(Duration::from_secs_f64(secs));
                }
            }
        }
    }
    None
}

// Extract avg from "rtt min/avg/max/mdev = a/b/c/d ms"
fn parse_rtt_avg_ms(s: &str) -> Option<Duration> {
    for line in s.lines() {
        let lower = line.to_lowercase();
        if lower.contains("rtt ") && lower.contains(" ms") && lower.contains('/') {
            // find part after '='
            if let Some(eq_idx) = lower.find('=') {
                let rest = lower[eq_idx + 1..].trim();
                // rest like "a/b/c/d ms"
                if let Some(ms_idx) = rest.find(" ms") {
                    let nums = &rest[..ms_idx];
                    let mut parts = nums.split('/');
                    let _min = parts.next()?;
                    let avg = parts.next()?;
                    if let Ok(v) = avg.trim().parse::<f64>() {
                        return Some(Duration::from_secs_f64(v / 1000.0));
                    }
                }
            }
        }
        // BSD/macOS line like: "round-trip min/avg/max/stddev = a/b/c/d ms"
        if (lower.contains("round-trip") || lower.contains("round trip"))
            && lower.contains(" ms")
            && lower.contains('/')
        {
            if let Some(eq_idx) = lower.find('=') {
                let rest = lower[eq_idx + 1..].trim();
                if let Some(ms_idx) = rest.find(" ms") {
                    let nums = &rest[..ms_idx];
                    let mut parts = nums.split('/');
                    let _min = parts.next()?;
                    let avg = parts.next()?;
                    if let Ok(v) = avg.trim().parse::<f64>() {
                        return Some(Duration::from_secs_f64(v / 1000.0));
                    }
                }
            }
        }
    }
    None
}
