mod config;
mod ping;
mod server;
mod speedtest;

use crate::config::Config;
use std::time::Duration;
use tokio::{signal, task};

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

    signal::ctrl_c().await?;
    eprintln!("Received CTRL-C, shutting down...");
    Ok(())
}
