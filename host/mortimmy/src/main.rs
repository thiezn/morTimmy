mod app;
mod audio;
mod brain;
mod camera;
mod cli;
mod config;
mod input;
mod nexo;
mod routing;
mod serial;
mod telemetry;
mod tui;
mod websocket;

use crate::cli::base::{Cli, Command};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse_args().command {
        Command::Ping(command) => app::ping(command).await,
        Command::Start(command) => app::start(command).await,
        Command::Config(command) => app::configure(command),
    }
}
