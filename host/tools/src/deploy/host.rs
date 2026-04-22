//! Host artifact build and installation workflows.

use std::path::Path;

use anyhow::{Context, Result, bail};

use super::cli::{HostBuildOptions, HostCommand, HostLocalInstallCommand, HostRemoteCopyCommand, HostRemoteInstallCommand, HostSubcommand};
use super::context::{CargoProfile, Workspace, host_artifact_path};
use super::process;

/// Execute a host deployment command.
pub fn run(workspace: &Workspace, command: HostCommand) -> Result<()> {
    match command.command {
        HostSubcommand::Build(command) => {
            build_host_artifact(workspace, &command.build)?;
            Ok(())
        }
        HostSubcommand::PrintArtifact(command) => {
            let artifact = build_host_artifact(workspace, &command.build)?;
            println!("{}", artifact.display());
            Ok(())
        }
        HostSubcommand::LocalInstall(command) => local_install(workspace, &command),
        HostSubcommand::RemoteCopy(command) => remote_copy(workspace, &command),
        HostSubcommand::RemoteInstall(command) => remote_install(workspace, &command),
    }
}

fn build_host_artifact(workspace: &Workspace, options: &HostBuildOptions) -> Result<std::path::PathBuf> {
    let profile = CargoProfile::from_name(options.profile.clone());
    let artifact_path = host_artifact_path(workspace, &profile, &options.bin);
    let mut command = std::process::Command::new("cargo");

    command
        .current_dir(workspace.root())
        .arg("build")
        .arg("-p")
        .arg(&options.package)
        .arg("--bin")
        .arg(&options.bin);
    for arg in profile.cargo_args() {
        command.arg(arg);
    }

    tracing::info!(
        package = %options.package,
        bin = %options.bin,
        profile = %profile.display_name(),
        artifact = %artifact_path.display(),
        "building host artifact"
    );

    process::run_checked(&mut command, "build host artifact")?;

    if artifact_path.is_file() {
        Ok(artifact_path)
    } else {
        bail!("expected host artifact not found at {}", artifact_path.display())
    }
}

fn local_install(workspace: &Workspace, command: &HostLocalInstallCommand) -> Result<()> {
    process::require_command("install")?;
    let artifact = build_host_artifact(workspace, &command.build)?;
    let destination = command.install_dir.join(&command.build.bin);
    let parent = destination.parent().context("install destination has no parent directory")?;

    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create install directory {}", parent.display()))?;

    tracing::info!(destination = %destination.display(), sudo = command.sudo, "installing host artifact locally");

    let mut install_command = if command.sudo {
        let mut command = std::process::Command::new("sudo");
        command.arg("install");
        command
    } else {
        std::process::Command::new("install")
    };

    install_command
        .current_dir(workspace.root())
        .arg("-m")
        .arg("755")
        .arg(&artifact)
        .arg(&destination);

    process::run_checked(&mut install_command, "install host artifact")
}

fn remote_copy(workspace: &Workspace, command: &HostRemoteCopyCommand) -> Result<()> {
    process::require_command("ssh")?;
    process::require_command("scp")?;
    let artifact = build_host_artifact(workspace, &command.build)?;
    let remote_target = command.remote_dir.join(&command.build.bin);

    tracing::info!(remote_host = %command.remote_host, remote_dir = %command.remote_dir.display(), "copying host artifact to remote machine");

    let mut mkdir_command = std::process::Command::new("ssh");
    mkdir_command
        .current_dir(workspace.root())
        .arg(&command.remote_host)
        .arg(format!("mkdir -p {}", shell_quote(command.remote_dir.as_path())));
    process::run_checked(&mut mkdir_command, "create remote staging directory")?;

    let mut scp_command = std::process::Command::new("scp");
    scp_command
        .current_dir(workspace.root())
        .arg(&artifact)
        .arg(format!("{}:{}", command.remote_host, remote_target.display()));
    process::run_checked(&mut scp_command, "copy host artifact to remote machine")
}

fn remote_install(workspace: &Workspace, command: &HostRemoteInstallCommand) -> Result<()> {
    remote_copy(
        workspace,
        &HostRemoteCopyCommand {
            build: command.build.clone(),
            remote_host: command.remote_host.clone(),
            remote_dir: command.remote_dir.clone(),
        },
    )?;

    let remote_source = command.remote_dir.join(&command.build.bin);
    let remote_destination = command.install_dir.join(&command.build.bin);
    tracing::info!(remote_host = %command.remote_host, destination = %remote_destination.display(), sudo = command.sudo, "installing host artifact on remote machine");

    let install_invocation = if command.sudo {
        format!(
            "sudo install -m 755 {} {}",
            shell_quote(remote_source.as_path()),
            shell_quote(remote_destination.as_path()),
        )
    } else {
        format!(
            "install -m 755 {} {}",
            shell_quote(remote_source.as_path()),
            shell_quote(remote_destination.as_path()),
        )
    };

    let mut ssh_command = std::process::Command::new("ssh");
    ssh_command
        .current_dir(workspace.root())
        .arg(&command.remote_host)
        .arg(install_invocation);
    process::run_checked(&mut ssh_command, "install host artifact on remote machine")
}

fn shell_quote(path: &Path) -> String {
    let rendered = path.display().to_string();
    format!("'{}'", rendered.replace('\'', "'\\''"))
}
