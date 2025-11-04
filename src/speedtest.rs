use heck::ToSnakeCase;
use prometheus::{
    Gauge, GaugeVec, HistogramVec, IntCounterVec, register_gauge, register_gauge_vec, register_histogram_vec,
    register_int_counter_vec,
};
use serde::Deserialize;
use serde_with::{DurationMilliSeconds, DurationMilliSecondsWithFrac, serde_as};
use std::sync::LazyLock;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::{sleep, timeout};

static SPEED: LazyLock<GaugeVec> =
    LazyLock::new(|| register_gauge_vec!("speed_bps", "speed in bytes per second", &["direction"]).unwrap());

static LATENCY: LazyLock<HistogramVec> = LazyLock::new(|| {
    register_histogram_vec!(
        "speedtest_latency_seconds",
        "speed test latency in seconds",
        &["state", "stat"],
    )
    .unwrap()
});

static BYTES: LazyLock<GaugeVec> =
    LazyLock::new(|| register_gauge_vec!("speedtest_bytes", "speedtest bytes transferred", &["direction"]).unwrap());

static ELAPSED: LazyLock<GaugeVec> = LazyLock::new(|| {
    register_gauge_vec!("speedtest_elapsed_seconds", "elapsed time in seconds", &["direction"]).unwrap()
});

static PACKET_LOSS: LazyLock<Gauge> =
    LazyLock::new(|| register_gauge!("speedtest_packet_loss", "packet loss percentage").unwrap());

static FAILS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!("speedtest_fails", "number of failed speed tests", &["error_type"]).unwrap()
});

pub async fn run(delay: Duration, timeout: Duration) {
    loop {
        match speedtest(timeout).await {
            Ok(result) => {
                println!(
                    "speedtest performed against {}:{} ({}) - {} ({}, {})",
                    result.server.host,
                    result.server.port,
                    result.server.ip,
                    result.server.name,
                    result.server.location,
                    result.server.country
                );

                SPEED
                    .with_label_values(&["download"])
                    .set(result.download.bandwidth as f64);
                SPEED.with_label_values(&["upload"]).set(result.upload.bandwidth as f64);
                BYTES.with_label_values(&["download"]).set(result.download.bytes as f64);
                BYTES.with_label_values(&["upload"]).set(result.upload.bytes as f64);
                ELAPSED
                    .with_label_values(&["download"])
                    .set(result.download.elapsed.as_secs_f64());
                ELAPSED
                    .with_label_values(&["upload"])
                    .set(result.upload.elapsed.as_secs_f64());
                PACKET_LOSS.set(result.packet_loss);
                LATENCY
                    .with_label_values(&["idle", "latency"])
                    .observe(result.ping.latency.as_secs_f64());
                LATENCY
                    .with_label_values(&["idle", "low"])
                    .observe(result.ping.low.as_secs_f64());
                LATENCY
                    .with_label_values(&["idle", "high"])
                    .observe(result.ping.high.as_secs_f64());
                LATENCY
                    .with_label_values(&["idle", "jitter"])
                    .observe(result.ping.jitter.as_secs_f64());
                LATENCY
                    .with_label_values(&["download", "latency"])
                    .observe(result.download.latency.mean.as_secs_f64());
                LATENCY
                    .with_label_values(&["download", "low"])
                    .observe(result.download.latency.low.as_secs_f64());
                LATENCY
                    .with_label_values(&["download", "high"])
                    .observe(result.download.latency.high.as_secs_f64());
                LATENCY
                    .with_label_values(&["download", "jitter"])
                    .observe(result.download.latency.jitter.as_secs_f64());
                LATENCY
                    .with_label_values(&["upload", "latency"])
                    .observe(result.upload.latency.mean.as_secs_f64());
                LATENCY
                    .with_label_values(&["upload", "low"])
                    .observe(result.upload.latency.low.as_secs_f64());
                LATENCY
                    .with_label_values(&["upload", "high"])
                    .observe(result.upload.latency.high.as_secs_f64());
                LATENCY
                    .with_label_values(&["upload", "jitter"])
                    .observe(result.upload.latency.jitter.as_secs_f64());
            }
            Err(err) => {
                eprintln!("failed to perform speedtest: {}", err);
                FAILS.with_label_values(&[err.to_snake_case()]).inc();
            }
        }
        sleep(delay).await;
    }
}

#[serde_as]
#[derive(Deserialize)]
struct StreamLatency {
    #[serde(rename = "iqm")]
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    mean: Duration,
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    low: Duration,
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    high: Duration,
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    jitter: Duration,
}

#[serde_as]
#[derive(Deserialize)]
struct IdleLatency {
    #[serde(default = "default_latency")]
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    latency: Duration,
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    low: Duration,
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    high: Duration,
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    jitter: Duration,
}

#[serde_as]
#[derive(Deserialize)]
struct StreamResult {
    bandwidth: u64,
    bytes: u64,
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    elapsed: Duration,
    latency: StreamLatency,
}

#[derive(Deserialize)]
struct Server {
    host: String,
    ip: String,
    port: u16,
    name: String,
    location: String,
    country: String,
}

#[derive(Deserialize)]
struct SpeedtestResult {
    ping: IdleLatency,
    download: StreamResult,
    upload: StreamResult,
    #[serde(rename = "packetLoss", default = "default_packet_loss")]
    packet_loss: f64,
    server: Server,
}

fn default_packet_loss() -> f64 {
    0.0
}

fn default_latency() -> Duration {
    Duration::from_secs(0)
}

async fn speedtest(timeout_dur: Duration) -> Result<SpeedtestResult, String> {
    let args = ["--accept-license", "--accept-gdpr", "--format=json"];
    let command = Command::new("./speedtest").args(args).output();

    let output = timeout(timeout_dur, command)
        .await
        .map_err(|_| "timeout".to_string())?
        .map_err(|e| format!("io_error_{}", e.kind().to_string().to_snake_case()))?;

    if !output.status.success() {
        return Err("nonzero_exit".to_string());
    }

    let out = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&out).map_err(|e| format!("json_error_{}", e.to_string().to_snake_case()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json() {
        let input = r#"{
            "type": "result",
            "timestamp": "2025-10-26T11:36:26Z",
            "ping": {
                "jitter": 3.245,
                "low": 13.379,
                "high": 21.874
            },
            "download": {
                "bandwidth": 115116205,
                "bytes": 1162635603,
                "elapsed": 10312,
                "latency": {
                    "iqm": 69.730,
                    "low": 11.473,
                    "high": 395.030,
                    "jitter": 24.770
                }
            },
            "upload": {
                "bandwidth": 12071584,
                "bytes": 97041069,
                "elapsed": 8101,
                "latency": {
                    "iqm": 177.390,
                    "low": 10.735,
                    "high": 322.531,
                    "jitter": 52.124
                }
            },
            "server": {
                "id": 52365,
                "host": "speedtest.ams.t-mobile.nl",
                "port": 8080,
                "name": "Odido",
                "location": "Amsterdam",
                "country": "Netherlands",
                "ip": "2a02:4240::e"
            }
        }"#;
        let result: SpeedtestResult = serde_json::from_str(input).unwrap();
        assert_eq!(result.ping.latency, Duration::from_micros(17634));
        assert_eq!(result.download.bandwidth, 115116205);
        assert_eq!(result.upload.bandwidth, 12071584);
        assert_eq!(result.packet_loss, 0.0);
        assert_eq!(result.server.host, "speedtest.ams.t-mobile.nl");
        assert_eq!(result.server.port, 8080);
        assert_eq!(result.server.name, "Odido");
        assert_eq!(result.server.location, "Amsterdam");
    }
}
