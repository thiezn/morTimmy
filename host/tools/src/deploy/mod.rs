//! Deployment command tree for host and firmware artifacts.

mod bootsel;
mod cli;
mod context;
mod firmware;
mod host;
mod probe;
mod process;
mod uf2;

use anyhow::Result;

pub use self::cli::DeployCommand;

/// Dispatch the deploy command tree against the current workspace.
pub fn run(command: DeployCommand) -> Result<()> {
    let workspace = context::Workspace::discover()?;

    match command.command {
        cli::DeployArea::Host(command) => host::run(&workspace, command),
        cli::DeployArea::Firmware(command) => firmware::run(&workspace, command),
    }
}
