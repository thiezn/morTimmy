//! Native probe-rs integration used by firmware flashing workflows.

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use mortimmy_deploy::FirmwareTarget;
use probe_rs::Permissions;
use probe_rs::flashing::{DownloadOptions, ElfOptions, FlashProgress, Format, download_file_with_options};
use probe_rs::probe::DebugProbeInfo;
use probe_rs::probe::list::Lister;

use super::cli::{FirmwareFlashCommand, ProbeOptions};

/// List all attached debug probes visible to probe-rs.
pub fn list_probes() -> Vec<DebugProbeInfo> {
    Lister::new().list_all()
}

/// Open a probe session against the selected firmware target.
pub fn open_session(target: &FirmwareTarget, options: &ProbeOptions, allow_erase_all: bool) -> Result<probe_rs::Session> {
    let probe_info = select_probe_info(options.probe_index)?;
    let mut probe = probe_info
        .open()
        .with_context(|| format!("failed to open debug probe {probe_info}"))?;

    probe
        .select_protocol(options.protocol.into())
        .with_context(|| format!("failed to select {:?} on debug probe", options.protocol))?;

    if let Some(speed_khz) = options.speed_khz {
        probe
            .set_speed(speed_khz)
            .with_context(|| format!("failed to set debug probe speed to {speed_khz} kHz"))?;
    }

    let permissions = if allow_erase_all {
        Permissions::new().allow_erase_all()
    } else {
        Permissions::default()
    };

    probe
        .attach(target.probe.chip, permissions)
        .with_context(|| format!("failed to attach debug probe to chip {}", target.probe.chip))
}

/// Flash an ELF file using the probe-rs library.
pub fn flash_elf(target: &FirmwareTarget, elf_path: &Path, command: &FirmwareFlashCommand) -> Result<()> {
    let start = Instant::now();
    let mut session = open_session(target, &command.probe, command.chip_erase)?;
    let mut download_options = DownloadOptions::default();
    download_options.do_chip_erase = command.chip_erase;
    download_options.preverify = command.preverify;
    download_options.verify = command.verify;
    download_options.keep_unwritten_bytes = command.restore_unwritten;
    download_options.disable_double_buffering = command.disable_double_buffering;
    download_options.progress = FlashProgress::new(|event| {
        tracing::debug!(?event, "probe-rs flash progress event");
    });

    tracing::info!(
        chip = target.probe.chip,
        elf = %elf_path.display(),
        chip_erase = command.chip_erase,
        preverify = command.preverify,
        verify = command.verify,
        "flashing firmware through debug probe"
    );

    download_file_with_options(
        &mut session,
        elf_path,
        Format::Elf(ElfOptions::default()),
        download_options,
    )
    .with_context(|| format!("failed to flash ELF {}", elf_path.display()))?;

    tracing::info!(elapsed_ms = start.elapsed().as_millis(), "firmware flashing completed");
    Ok(())
}

/// Select a single probe, either by explicit index or by requiring exactly one attached probe.
pub fn select_probe_info(probe_index: Option<usize>) -> Result<DebugProbeInfo> {
    let probes = list_probes();

    match probe_index {
        Some(index) => probes
            .get(index)
            .cloned()
            .ok_or_else(|| anyhow!("probe index {} is out of range; run `mortimmy-tools deploy firmware probe-list` first", index)),
        None if probes.is_empty() => bail!("No debug probes were found."),
        None if probes.len() == 1 => Ok(probes[0].clone()),
        None => {
            let mut rendered = String::from("Multiple debug probes were found. Re-run with --probe-index <N>.\n\n");
            for (index, probe) in probes.iter().enumerate() {
                rendered.push_str(&format!("[{index}] {probe}\n"));
            }
            bail!(rendered)
        }
    }
}

/// Render a `--probe` selector string accepted by the `probe-rs` CLI.
pub fn selector_string(probe: &DebugProbeInfo) -> String {
    let base = format!("{:04x}:{:04x}", probe.vendor_id, probe.product_id);
    match probe.serial_number.as_ref() {
        Some(serial) if !serial.is_empty() => format!("{base}:{serial}"),
        _ => base,
    }
}
