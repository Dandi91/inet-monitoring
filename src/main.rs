mod ping;
mod server;

use crate::ping::run;
use crate::server::serve;
use std::env;
use std::time::Duration;

fn main() {
    let port = env::var("PORT")
        .unwrap_or("9090".to_string())
        .parse::<u16>()
        .expect("invalid port");
    let targets = env::var("TARGETS")
        .unwrap_or("8.8.8.8".to_string())
        .split(',')
        .map(String::from)
        .collect();
    let delay = env::var("DELAY")
        .unwrap_or("5".to_string())
        .parse::<f32>()
        .expect("invalid delay");
    let timeout = env::var("TIMEOUT")
        .unwrap_or("5".to_string())
        .parse::<f32>()
        .expect("invalid timeout");

    let pinger = run(
        targets,
        Duration::from_secs_f32(delay),
        Duration::from_secs_f32(timeout),
    );
    let server = serve(port);
    server.join().unwrap();
    pinger.join().unwrap();
}
