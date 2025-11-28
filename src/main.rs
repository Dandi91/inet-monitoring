mod config;
mod ping;
mod server;
mod speedtest;

use crate::config::Config;
use std::io;
use std::time::Duration;
use tokio::signal::unix::{SignalKind, signal};
use tokio::{select, task};

async fn wait_for_signal() -> io::Result<()> {
    let mut terminate = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    select! {
        _ = terminate.recv() => Ok(()),
        _ = interrupt.recv() => Ok(()),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load();

    task::spawn(server::serve(config.port));
    task::spawn(ping::run(
        config.targets,
        Duration::from_secs_f32(config.delay),
        Duration::from_secs_f32(config.timeout),
    ));
    task::spawn(speedtest::run(
        Duration::from_secs_f32(config.speedtest_interval),
        Duration::from_secs_f32(config.speedtest_timeout),
    ));

    wait_for_signal().await?;
    eprintln!("Received signal, shutting down...");
    Ok(())
}
