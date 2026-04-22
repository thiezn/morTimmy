//! CLI definitions for mortimmy operational tools.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::deploy::DeployCommand;

/// Log level accepted by the operational tooling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// Render the level in the format expected by `tracing_subscriber`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "mortimmy-tools",
    about = "Operational tooling for the mortimmy robot",
    long_about = "Operational tooling for the mortimmy robot, including capture inspection, replay helpers, host deployment, and RP2350 firmware build and flashing workflows."
)]
pub struct Cli {
    /// Default log level used when `RUST_LOG` is not set.
    #[arg(long, global = true, value_enum, default_value_t = LogLevel::Info, help_heading = "GLOBAL OPTIONS")]
    pub log_level: LogLevel,
    /// Disable ANSI color in log output.
    #[arg(long, global = true, help_heading = "GLOBAL OPTIONS")]
    pub no_color: bool,
    /// Selected tooling command.
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
    /// Build, flash, install, or deploy mortimmy host and firmware artifacts.
    Deploy(DeployCommand),
    /// Inspect a recorded framed capture.
    Inspect(InspectCommand),
    /// Replay a recorded framed capture.
    Replay(ReplayCommand),
}

#[derive(Debug, Args)]
pub struct InspectCommand {
    /// Path to the capture file to inspect.
    #[arg(value_name = "CAPTURE")]
    pub input: PathBuf,
}

#[derive(Debug, Args)]
pub struct ReplayCommand {
    /// Path to the capture file to replay.
    #[arg(value_name = "CAPTURE")]
    pub input: PathBuf,
    /// Validate the capture without sending it to hardware.
    #[arg(long, help = "Validate the capture without sending it to hardware")]
    pub dry_run: bool,
}
