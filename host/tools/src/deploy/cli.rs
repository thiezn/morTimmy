//! Clap definitions for the deploy subcommands.

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

use super::context::FirmwareTargetId;

/// Top-level deployment entrypoint.
#[derive(Debug, Args)]
pub struct DeployCommand {
    /// Deployment area to operate on.
    #[command(subcommand)]
    pub command: DeployArea,
}

/// Deployment areas supported by the CLI.
#[derive(Debug, Subcommand)]
pub enum DeployArea {
    /// Build or install the mortimmy host daemon.
    Host(HostCommand),
    /// Build, package, and flash the RP2350 firmware.
    Firmware(FirmwareCommand),
}

/// Host deployment command tree.
#[derive(Debug, Args)]
pub struct HostCommand {
    /// Host deployment action.
    #[command(subcommand)]
    pub command: HostSubcommand,
}

/// Host deployment actions.
#[derive(Debug, Subcommand)]
pub enum HostSubcommand {
    /// Build the host daemon artifact.
    Build(HostBuildCommand),
    /// Build the host daemon and print the artifact path.
    PrintArtifact(HostBuildCommand),
    /// Build the host daemon and install it locally.
    LocalInstall(HostLocalInstallCommand),
    /// Build the host daemon and copy it to a remote machine.
    RemoteCopy(HostRemoteCopyCommand),
    /// Build the host daemon, copy it remotely, and install it on the target host.
    RemoteInstall(HostRemoteInstallCommand),
}

/// Common build options for the host daemon.
#[derive(Debug, Args, Clone)]
pub struct HostBuildOptions {
    /// Cargo package that owns the host daemon binary.
    #[arg(long, default_value = "mortimmy", help_heading = "HOST BUILD")]
    pub package: String,
    /// Binary name to build and deploy.
    #[arg(long, default_value = "mortimmy", help_heading = "HOST BUILD")]
    pub bin: String,
    /// Cargo profile name used for the host artifact.
    #[arg(long, default_value = "release", help_heading = "HOST BUILD")]
    pub profile: String,
}

/// Build-only host command.
#[derive(Debug, Args, Clone)]
pub struct HostBuildCommand {
    #[command(flatten)]
    pub build: HostBuildOptions,
}

/// Local host installation command.
#[derive(Debug, Args, Clone)]
pub struct HostLocalInstallCommand {
    #[command(flatten)]
    pub build: HostBuildOptions,
    /// Destination directory for the installed binary.
    #[arg(long, default_value = "/usr/local/bin", help_heading = "HOST INSTALL")]
    pub install_dir: PathBuf,
    /// Invoke `sudo install` instead of plain `install`.
    #[arg(long, help_heading = "HOST INSTALL")]
    pub sudo: bool,
}

/// Remote copy command.
#[derive(Debug, Args, Clone)]
pub struct HostRemoteCopyCommand {
    #[command(flatten)]
    pub build: HostBuildOptions,
    /// SSH destination such as `pi@raspberrypi.local`.
    #[arg(long, help_heading = "REMOTE HOST")]
    pub remote_host: String,
    /// Remote staging directory used before installation.
    #[arg(long, default_value = "/tmp/mortimmy", help_heading = "REMOTE HOST")]
    pub remote_dir: PathBuf,
}

/// Remote install command.
#[derive(Debug, Args, Clone)]
pub struct HostRemoteInstallCommand {
    #[command(flatten)]
    pub build: HostBuildOptions,
    /// SSH destination such as `pi@raspberrypi.local`.
    #[arg(long, help_heading = "REMOTE HOST")]
    pub remote_host: String,
    /// Remote staging directory used before installation.
    #[arg(long, default_value = "/tmp/mortimmy", help_heading = "REMOTE HOST")]
    pub remote_dir: PathBuf,
    /// Destination directory on the remote machine.
    #[arg(long, default_value = "/usr/local/bin", help_heading = "REMOTE HOST")]
    pub install_dir: PathBuf,
    /// Invoke `sudo install` on the remote host.
    #[arg(long, help_heading = "REMOTE HOST")]
    pub sudo: bool,
}

/// Firmware deployment command tree.
#[derive(Debug, Args)]
pub struct FirmwareCommand {
    /// Firmware deployment action.
    #[command(subcommand)]
    pub command: FirmwareSubcommand,
}

/// Firmware deployment actions.
#[derive(Debug, Subcommand)]
pub enum FirmwareSubcommand {
    /// Build the firmware ELF.
    Build(FirmwareBuildCommand),
    /// Build the firmware ELF and print the artifact path.
    PrintArtifact(FirmwareBuildCommand),
    /// Build the firmware ELF, convert it to UF2, and print the UF2 path.
    Uf2(FirmwareUf2Command),
    /// Build a UF2 and deploy it through BOOTSEL.
    Uf2Deploy(FirmwareUf2Command),
    /// List attached debug probes visible to probe-rs.
    ProbeList(FirmwareProbeListCommand),
    /// Build the firmware ELF and flash it through a debug probe using the probe-rs library.
    Flash(FirmwareFlashCommand),
    /// Build the firmware ELF and hand off to `probe-rs run` for defmt/RTT monitoring.
    Run(FirmwareRunCommand),
}

/// Selects which firmware target metadata block to use.
#[derive(Debug, Args, Clone)]
pub struct FirmwareTargetOptions {
    /// Firmware target definition owned by the firmware crate.
    #[arg(long, value_enum, default_value_t = FirmwareTargetId::MotionController, help_heading = "FIRMWARE TARGET")]
    pub target: FirmwareTargetId,
    /// Override the firmware Cargo profile.
    #[arg(long, help_heading = "FIRMWARE TARGET")]
    pub profile: Option<String>,
}

/// Shared options for selecting and configuring a debug probe.
#[derive(Debug, Args, Clone)]
pub struct ProbeOptions {
    /// Index from `probe-list` to use when more than one debug probe is attached.
    #[arg(long, help_heading = "DEBUG PROBE")]
    pub probe_index: Option<usize>,
    /// Wire protocol used to talk to the target MCU.
    #[arg(long, value_enum, default_value_t = ProbeWireProtocol::Swd, help_heading = "DEBUG PROBE")]
    pub protocol: ProbeWireProtocol,
    /// Debug probe speed in kHz.
    #[arg(long, help_heading = "DEBUG PROBE")]
    pub speed_khz: Option<u32>,
}

/// Probe wire protocols supported by the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProbeWireProtocol {
    Swd,
    Jtag,
}

impl ProbeWireProtocol {
    /// Render the protocol in the form expected by the `probe-rs` CLI.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Swd => "swd",
            Self::Jtag => "jtag",
        }
    }
}

impl From<ProbeWireProtocol> for probe_rs::probe::WireProtocol {
    fn from(value: ProbeWireProtocol) -> Self {
        match value {
            ProbeWireProtocol::Swd => Self::Swd,
            ProbeWireProtocol::Jtag => Self::Jtag,
        }
    }
}

/// Build-only firmware command.
#[derive(Debug, Args, Clone)]
pub struct FirmwareBuildCommand {
    #[command(flatten)]
    pub target: FirmwareTargetOptions,
}

/// UF2 build and deploy command.
#[derive(Debug, Args, Clone)]
pub struct FirmwareUf2Command {
    #[command(flatten)]
    pub target: FirmwareTargetOptions,
    /// Optional output path for the generated UF2.
    #[arg(long, value_name = "UF2", help_heading = "UF2 BUILD")]
    pub output: Option<PathBuf>,
}

/// Probe listing command.
#[derive(Debug, Args, Clone, Default)]
pub struct FirmwareProbeListCommand {}

/// Firmware flash command using the probe-rs library.
#[derive(Debug, Args, Clone)]
pub struct FirmwareFlashCommand {
    #[command(flatten)]
    pub target: FirmwareTargetOptions,
    #[command(flatten)]
    pub probe: ProbeOptions,
    /// Perform a full chip erase when the target supports it.
    #[arg(long, help_heading = "FLASH CONFIGURATION")]
    pub chip_erase: bool,
    /// Before flashing, compare the image to flash contents and skip up-to-date pages.
    #[arg(long, help_heading = "FLASH CONFIGURATION")]
    pub preverify: bool,
    /// After flashing, verify the written flash contents.
    #[arg(long, help_heading = "FLASH CONFIGURATION")]
    pub verify: bool,
    /// Restore bytes from partially overwritten sectors.
    #[arg(long, help_heading = "FLASH CONFIGURATION")]
    pub restore_unwritten: bool,
    /// Disable double buffering while programming flash.
    #[arg(long, help_heading = "FLASH CONFIGURATION")]
    pub disable_double_buffering: bool,
}

/// Firmware run command that delegates the monitor UX to the `probe-rs` CLI.
#[derive(Debug, Args, Clone)]
pub struct FirmwareRunCommand {
    #[command(flatten)]
    pub target: FirmwareTargetOptions,
    #[command(flatten)]
    pub probe: ProbeOptions,
}
