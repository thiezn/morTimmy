use clap::{Parser, Subcommand};

use crate::cli::{config::ConfigCommand, start::StartCommand};

#[derive(Debug, Parser)]
#[command(name = "mortimmy-pi-daemon", about = "Host bridge scaffold for the mortimmy robot")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Start(StartCommand),
    Config(ConfigCommand),
}
