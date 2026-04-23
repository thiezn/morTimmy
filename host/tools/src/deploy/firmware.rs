//! Firmware build, packaging, and flashing workflows.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use super::bootsel::{self, BootselTransport};
use super::cli::{FirmwareBuildCommand, FirmwareCommand, FirmwareFlashCommand, FirmwareProbeListCommand, FirmwareRunCommand, FirmwareSubcommand, FirmwareUf2Command};
use super::context::{CargoProfile, Workspace, default_firmware_uf2_path, firmware_elf_path};
use super::probe;
use super::process;
use super::uf2;

/// Execute a firmware deployment command.
pub fn run(workspace: &Workspace, command: FirmwareCommand) -> Result<()> {
    match command.command {
        FirmwareSubcommand::Build(command) => {
            build_firmware_artifact(workspace, &command)?;
            Ok(())
        }
        FirmwareSubcommand::PrintArtifact(command) => {
            let artifact = build_firmware_artifact(workspace, &command)?;
            println!("{}", artifact.display());
            Ok(())
        }
        FirmwareSubcommand::Uf2(command) => {
            let artifact = build_firmware_uf2(workspace, &command)?;
            println!("{}", artifact.display());
            Ok(())
        }
        FirmwareSubcommand::Uf2Deploy(command) => deploy_firmware_uf2(workspace, &command),
        FirmwareSubcommand::ProbeList(command) => {
            list_debug_probes(command);
            Ok(())
        }
        FirmwareSubcommand::Flash(command) => flash_firmware(workspace, &command),
        FirmwareSubcommand::Run(command) => run_firmware(workspace, &command),
    }
}

fn build_firmware_artifact(workspace: &Workspace, command: &FirmwareBuildCommand) -> Result<PathBuf> {
    let target = command.target.target.metadata();
    let profile = CargoProfile::from_cli_or_default(command.target.profile.as_ref(), target.artifact.default_profile);
    let manifest_path = workspace.path(target.artifact.manifest_path);
    let cargo_target_dir = workspace.path(target.artifact.cargo_target_dir);
    let elf_path = firmware_elf_path(workspace, target, &profile);
    let mut build_command = std::process::Command::new("cargo");

    build_command
        .current_dir(workspace.root())
        .arg("build")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--bin")
        .arg(target.artifact.bin_name)
        .arg("--target-dir")
        .arg(&cargo_target_dir)
        .arg("--target")
        .arg(target.artifact.target_triple);

    if target.artifact.cargo_no_default_features {
        build_command.arg("--no-default-features");
    }
    if !target.artifact.cargo_features.is_empty() {
        build_command
            .arg("--features")
            .arg(target.artifact.cargo_features.join(","));
    }
    for arg in profile.cargo_args() {
        build_command.arg(arg);
    }

    tracing::info!(
        target = target.id,
        board = target.board_name,
        bin = target.artifact.bin_name,
        features = ?target.artifact.cargo_features,
        profile = %profile.display_name(),
        cargo_target_dir = %cargo_target_dir.display(),
        elf = %elf_path.display(),
        "building firmware artifact"
    );

    process::run_checked(&mut build_command, "build firmware artifact")?;

    if elf_path.is_file() {
        Ok(elf_path)
    } else {
        bail!("expected firmware artifact not found at {}", elf_path.display())
    }
}

fn build_firmware_uf2(workspace: &Workspace, command: &FirmwareUf2Command) -> Result<PathBuf> {
    process::require_command("elf2uf2-rs")?;
    let target = command.target.target.metadata();
    let elf_path = build_firmware_artifact(
        workspace,
        &FirmwareBuildCommand {
            target: command.target.clone(),
        },
    )?;
    let profile = CargoProfile::from_cli_or_default(command.target.profile.as_ref(), target.artifact.default_profile);
    let uf2_path = resolve_uf2_output(workspace, target, &profile, command.output.as_deref())?;
    if let Some(parent) = uf2_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create UF2 output directory {}", parent.display()))?;
    }

    tracing::info!(elf = %elf_path.display(), uf2 = %uf2_path.display(), "converting ELF to UF2");
    let mut elf2uf2_command = std::process::Command::new("elf2uf2-rs");
    elf2uf2_command
        .current_dir(workspace.root())
        .arg(&elf_path)
        .arg(&uf2_path);
    process::run_checked(&mut elf2uf2_command, "convert ELF to UF2")?;

    let patch_summary = uf2::patch_uf2(&uf2_path, target.uf2.family_id, target.uf2.absolute_block_location)?;
    tracing::info!(
        uf2 = %uf2_path.display(),
        family = target.uf2.family_name,
        family_id = format_args!("0x{:08x}", target.uf2.family_id),
        patched_blocks = patch_summary.patched_blocks,
        absolute_block_added = patch_summary.absolute_block_added,
        "patched UF2 metadata"
    );

    Ok(uf2_path)
}

fn deploy_firmware_uf2(workspace: &Workspace, command: &FirmwareUf2Command) -> Result<()> {
    let target = command.target.target.metadata();
    let uf2_path = build_firmware_uf2(workspace, command)?;

    match bootsel::detect_transport(&target.bootsel)? {
        Some(BootselTransport::Picotool) => {
            process::require_command("picotool")?;
            tracing::info!(uf2 = %uf2_path.display(), "deploying firmware over picotool");
            let mut picotool_command = std::process::Command::new("picotool");
            picotool_command
                .current_dir(workspace.root())
                .arg("load")
                .arg("-v")
                .arg("-x")
                .arg("-t")
                .arg("uf2")
                .arg(&uf2_path);
            process::run_checked(&mut picotool_command, "deploy firmware over picotool")
        }
        Some(BootselTransport::Volume(volume)) => {
            tracing::info!(volume = %volume.display(), uf2 = %uf2_path.display(), "deploying firmware over BOOTSEL mass-storage volume");
            let destination = volume.join("NEW.UF2");
            std::fs::copy(&uf2_path, &destination).with_context(|| {
                format!(
                    "failed to copy UF2 {} to mounted BOOTSEL volume {}",
                    uf2_path.display(),
                    destination.display()
                )
            })?;
            if process::command_exists("sync") {
                let mut sync_command = std::process::Command::new("sync");
                sync_command.current_dir(workspace.root());
                process::run_checked(&mut sync_command, "flush filesystem buffers")?;
            }
            Ok(())
        }
        None => bail!(
            "Unable to find a board in BOOTSEL mode via picotool or a mounted UF2 volume.\n\n{}",
            bootsel::instructions(
                &target.bootsel,
                "cargo run -p mortimmy-tools -- deploy firmware uf2-deploy"
            )
        ),
    }
}

fn list_debug_probes(_command: FirmwareProbeListCommand) {
    let probes = probe::list_probes();
    if probes.is_empty() {
        println!("No debug probes were found.");
        return;
    }

    println!("The following debug probes were found:");
    for (index, probe) in probes.iter().enumerate() {
        println!("[{index}] {probe}");
    }
}

fn flash_firmware(workspace: &Workspace, command: &FirmwareFlashCommand) -> Result<()> {
    let target = command.target.target.metadata();
    let elf_path = build_firmware_artifact(
        workspace,
        &FirmwareBuildCommand {
            target: command.target.clone(),
        },
    )?;
    probe::flash_elf(target, &elf_path, command)
}

fn run_firmware(workspace: &Workspace, command: &FirmwareRunCommand) -> Result<()> {
    process::require_command("probe-rs")?;
    let target = command.target.target.metadata();
    let elf_path = build_firmware_artifact(
        workspace,
        &FirmwareBuildCommand {
            target: command.target.clone(),
        },
    )?;
    let selected_probe = probe::select_probe_info(command.probe.probe_index)?;
    let probe_selector = probe::selector_string(&selected_probe);
    let mut run_command = std::process::Command::new("probe-rs");

    tracing::info!(
        chip = target.probe.chip,
        probe = %probe_selector,
        elf = %elf_path.display(),
        "delegating firmware run workflow to probe-rs CLI"
    );

    run_command
        .current_dir(workspace.root())
        .arg("run")
        .arg("--chip")
        .arg(target.probe.chip)
        .arg("--probe")
        .arg(probe_selector)
        .arg("--protocol")
        .arg(command.probe.protocol.as_str());

    if let Some(speed_khz) = command.probe.speed_khz {
        run_command.arg("--speed").arg(speed_khz.to_string());
    }

    run_command.arg(&elf_path);
    process::run_checked(&mut run_command, "run firmware with probe-rs")
}

fn resolve_uf2_output(
    workspace: &Workspace,
    target: &mortimmy_deploy::FirmwareTarget,
    profile: &CargoProfile,
    output: Option<&Path>,
) -> Result<PathBuf> {
    match output {
        Some(path) => workspace.resolve_user_path(path),
        None => Ok(default_firmware_uf2_path(workspace, target, profile)),
    }
}
