mod config;
mod ping;
mod server;

use crate::config::Config;
use crate::ping::run;
use crate::server::serve;
use std::time::Duration;
use tokio::{signal, task};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load();

    task::spawn(serve(config.port));
    task::spawn(run(
        config.targets,
        Duration::from_secs_f32(config.delay),
        Duration::from_secs_f32(config.timeout),
    ));

    signal::ctrl_c().await?;
    eprintln!("Received CTRL-C, shutting down...");
    Ok(())
}
