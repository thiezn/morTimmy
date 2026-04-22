mod cli;
mod deploy;
mod inspect;
mod replay;
mod tracing_support;

use crate::cli::{Cli, Command};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse_args();
    tracing_support::init(cli.log_level, cli.no_color);

    match cli.command {
        Command::Deploy(command) => deploy::run(command),
        Command::Inspect(command) => inspect::run(command),
        Command::Replay(command) => replay::run(command),
    }
}
