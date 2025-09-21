use heck::ToSnakeCase;
use lazy_static::lazy_static;
use ping::{Error, SocketType};
use prometheus::{HistogramVec, IntCounterVec, register_histogram_vec, register_int_counter_vec};
use std::net::ToSocketAddrs;
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

lazy_static! {
    static ref PING_DELAY: HistogramVec =
        register_histogram_vec!("ping_delay_seconds", "ping delay in seconds", &["hostname"]).unwrap();
    static ref PING_FAILS: IntCounterVec =
        register_int_counter_vec!("ping_fails", "number of failed pings", &["hostname", "error_type"]).unwrap();
}

pub fn run(targets: Vec<String>, delay: Duration, timeout: Duration) -> JoinHandle<()> {
    thread::spawn(move || {
        loop {
            for target in &targets {
                match ping_target(target, timeout) {
                    Ok(latency) => PING_DELAY.with_label_values(&[target]).observe(latency.as_secs_f64()),
                    Err(err) => {
                        eprintln!("failed to ping {}: {}", target, err);
                        PING_FAILS
                            .with_label_values(&[
                                target,
                                &match err {
                                    Error::InvalidProtocol => "invalid_protocol".to_string(),
                                    Error::InternalError => "internal_error".to_string(),
                                    Error::DecodeV4Error => "decode_v4_error".to_string(),
                                    Error::DecodeEchoReplyError => "decode_echo_reply_error".to_string(),
                                    Error::IoError { error } => error.kind().to_string().to_snake_case(),
                                },
                            ])
                            .inc();
                    }
                }
            }
            thread::sleep(delay);
        }
    })
}

fn ping_target(target: &str, timeout: Duration) -> Result<Duration, Error> {
    let ip = (target, 0)
        .to_socket_addrs()?
        .next()
        .expect("hostname should have at least 1 ip")
        .ip();
    let start = Instant::now();
    ping::new(ip).socket_type(SocketType::RAW).timeout(timeout).send()?;
    Ok(start.elapsed())
}
