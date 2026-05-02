mod app;
mod audio;
mod brain;
mod camera;
mod cli;
mod config;
mod input;
mod nexo;
mod serial;
mod telemetry;
mod tui;
mod websocket;

use crate::cli::base::{Cli, Command};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse_args().command {
        Command::Start(command) => app::start(command).await,
        Command::Test(command) => app::test(command).await,
        Command::Config(command) => app::configure(command),
    }
}
