use heck::ToSnakeCase;
use lazy_static::lazy_static;
use prometheus::{HistogramVec, IntCounterVec, register_histogram_vec, register_int_counter_vec};
use regex::Regex;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::sleep;

lazy_static! {
    static ref PING_DELAY: HistogramVec =
        register_histogram_vec!("ping_delay_seconds", "ping delay in seconds", &["hostname"]).unwrap();
    static ref PING_FAILS: IntCounterVec =
        register_int_counter_vec!("ping_fails", "number of failed pings", &["hostname", "error_type"]).unwrap();
    static ref TIME_PATTERN: Regex = Regex::new(r"(?i)(?:rtt|round[- ]?trip).*=\s*(.+)/(.+)/(.+)(/.+)?\s*ms").unwrap();
}

pub async fn run(targets: Vec<String>, delay: Duration, timeout: Duration) {
    loop {
        for target in &targets {
            match ping_target(target, timeout).await {
                Ok(latency) => PING_DELAY.with_label_values(&[target]).observe(latency.as_secs_f64()),
                Err(err) => {
                    eprintln!("failed to ping {}: {}", target, err);
                    PING_FAILS.with_label_values(&[target, &err.to_snake_case()]).inc();
                }
            }
        }
        sleep(delay).await;
    }
}

/// Execute system `ping` and parse output for latency.
/// Supports common Linux/macOS ping formats.
/// Returns Err(...) with a short snake_case string describing the failure cause.
async fn ping_target(target: &str, timeout: Duration) -> Result<Duration, String> {
    #[cfg(target_os = "macos")]
    let timeout_arg = "-t";
    #[cfg(not(target_os = "macos"))]
    let timeout_arg = "-W";

    let timeout_value = timeout.as_secs().max(1).to_string();
    let args = ["-c", "1", timeout_arg, &timeout_value, target];

    let output = Command::new("ping")
        .args(args)
        .output()
        .await
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

    let out = String::from_utf8_lossy(&output.stdout);
    if let Some(dur) = parse_time_ms_from_output(&out) {
        println!("pinging {} took {:?}", target, dur);
        return Ok(dur);
    }

    Err("parse_error".to_string())
}

fn parse_time_ms_from_output(s: &str) -> Option<Duration> {
    TIME_PATTERN.captures(s).and_then(|caps| {
        caps.get(2)?
            .as_str()
            .parse::<f64>()
            .ok()
            .map(|ms| Duration::from_secs_f64(ms / 1000.0))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time_macos() {
        let s = r"PING 8.8.8.8 (8.8.8.8): 56 data bytes
64 bytes from 8.8.8.8: icmp_seq=0 ttl=117 time=19.379 ms

--- 8.8.8.8 ping statistics ---
1 packets transmitted, 1 packets received, 0.0% packet loss
round-trip min/avg/max/stddev = 19.379/19.379/19.379/nan ms";
        assert_eq!(
            parse_time_ms_from_output(s),
            Some(Duration::from_secs_f64(19.379 / 1000.0))
        );
    }

    #[test]
    fn test_parse_time_ms_linux() {
        let s = r"PING 8.8.8.8 (8.8.8.8) 56(84) bytes of data.
64 bytes from 8.8.8.8: icmp_seq=1 ttl=117 time=15.5 ms

--- 8.8.8.8 ping statistics ---
1 packets transmitted, 1 received, 0% packet loss, time 0ms
rtt min/avg/max/mdev = 15.502/15.502/15.502/0.000 ms";
        assert_eq!(
            parse_time_ms_from_output(s),
            Some(Duration::from_secs_f64(15.502 / 1000.0))
        );
    }

    #[test]
    fn test_parse_time_ms_busybox() {
        let s = r"PING 8.8.8.8 (8.8.8.8): 56 data bytes
64 bytes from 8.8.8.8: seq=0 ttl=116 time=16.636 ms

--- 8.8.8.8 ping statistics ---
1 packets transmitted, 1 packets received, 0% packet loss
round-trip min/avg/max = 16.636/16.636/16.636 ms";
        assert_eq!(
            parse_time_ms_from_output(s),
            Some(Duration::from_secs_f64(16.636 / 1000.0))
        );
    }
}
