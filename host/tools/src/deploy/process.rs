//! Process helpers shared by deployment commands.

use std::ffi::OsStr;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// Ensure a command exists on the current `PATH`.
pub fn require_command(name: &str) -> Result<()> {
    if command_exists(name) {
        Ok(())
    } else {
        bail!("missing required command: {name}")
    }
}

/// Return whether a command exists on the current `PATH`.
pub fn command_exists(name: &str) -> bool {
    let path_var = match std::env::var_os("PATH") {
        Some(path) => path,
        None => return false,
    };

    std::env::split_paths(&path_var).any(|entry| entry.join(name).is_file())
}

/// Run a command with inherited stdio and require success.
pub fn run_checked(command: &mut Command, description: &str) -> Result<()> {
    tracing::debug!(command = %format_command(command), %description, "running external command");
    let status = command.status().with_context(|| format!("failed to spawn {description}"))?;

    if status.success() {
        Ok(())
    } else {
        bail!("{description} failed with status {status}")
    }
}

/// Return whether a command succeeds while suppressing its output.
pub fn status_success(command: &mut Command, description: &str) -> Result<bool> {
    tracing::debug!(command = %format_command(command), %description, "checking command status");
    let status = command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("failed to spawn {description}"))?;
    Ok(status.success())
}

fn format_command(command: &Command) -> String {
    let program = command.get_program().to_string_lossy();
    let args = command
        .get_args()
        .map(os_str_to_string)
        .collect::<Vec<_>>()
        .join(" ");

    if args.is_empty() {
        program.into_owned()
    } else {
        format!("{} {}", program, args)
    }
}

fn os_str_to_string(value: &OsStr) -> String {
    let rendered = value.to_string_lossy();
    if rendered.contains(' ') {
        format!("\"{rendered}\"")
    } else {
        rendered.into_owned()
    }
}
