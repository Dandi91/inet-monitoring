mod ping;
mod server;

use crate::ping::run;
use crate::server::serve;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::iterator::Signals;
use std::time::Duration;
use std::{env, thread};

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let (shutdown_tx, shutdown_rx) = crossbeam_channel::unbounded();
    let mut signals = Signals::new(TERM_SIGNALS)?;
    thread::spawn(move || {
        if let Some(signal) = signals.forever().next() {
            eprintln!("Received signal {:?}, initiating shutdown...", signal);
            // Dropping the sender will cause all receivers to get a
            // `Disconnected` error, signaling them to shut down.
            drop(shutdown_tx);
        }
    });

    let pinger = run(
        targets,
        Duration::from_secs_f32(delay),
        Duration::from_secs_f32(timeout),
        shutdown_rx.clone(),
    );
    let server = serve(port, shutdown_rx);
    server.join().unwrap();
    pinger.join().unwrap();

    Ok(())
}
