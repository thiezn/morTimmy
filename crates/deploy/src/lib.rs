#![no_std]

//! Shared deployment metadata owned by firmware crates and consumed by host tooling.

/// Immutable deployment definition for a firmware target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FirmwareTarget {
    /// Stable target identifier used by the CLI.
    pub id: &'static str,
    /// Human-readable board name.
    pub board_name: &'static str,
    /// Human-readable MCU identifier.
    pub board_mcu: &'static str,
    /// Build inputs for the firmware artifact.
    pub artifact: Artifact,
    /// Debug-probe flashing configuration.
    pub probe: Probe,
    /// UF2 post-processing configuration.
    pub uf2: Uf2,
    /// BOOTSEL detection and operator guidance.
    pub bootsel: Bootsel,
}

/// Firmware build inputs shared across deploy subcommands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Artifact {
    /// Cargo manifest path for the firmware crate, relative to the workspace root.
    pub manifest_path: &'static str,
    /// Cargo package name for the firmware crate.
    pub package_name: &'static str,
    /// Binary name emitted by the firmware crate.
    pub bin_name: &'static str,
    /// Cargo feature flags that define this concrete firmware image.
    pub cargo_features: &'static [&'static str],
    /// Whether tooling should disable default features before enabling `cargo_features`.
    pub cargo_no_default_features: bool,
    /// Workspace-relative target directory used to isolate feature-specific artifacts.
    pub cargo_target_dir: &'static str,
    /// Embedded target triple for the firmware artifact.
    pub target_triple: &'static str,
    /// Default build profile for deploy-oriented commands.
    pub default_profile: BuildProfile,
}

/// Small fixed set of supported cargo build profiles used by deploy metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildProfile {
    /// Use Cargo's debug profile.
    Debug,
    /// Use Cargo's release profile.
    Release,
}

impl BuildProfile {
    /// Render the profile in the same shape used by CLI flags and artifact paths.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }
}

/// Debug-probe specific flashing metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Probe {
    /// probe-rs chip name used for flashing and RTT attach.
    pub chip: &'static str,
}

/// UF2 generation and post-processing metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Uf2 {
    /// Human-readable UF2 family name.
    pub family_name: &'static str,
    /// UF2 family identifier value.
    pub family_id: u32,
    /// Optional RP2350 absolute ignore block location.
    pub absolute_block_location: Option<u32>,
}

/// BOOTSEL detection hints and operator instructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bootsel {
    /// Printed button name used to enter the board bootloader.
    pub button_name: &'static str,
    /// Known BOOTSEL volume labels.
    pub volume_labels: &'static [&'static str],
    /// Tokens expected inside `INFO_UF2.TXT`.
    pub info_tokens: &'static [&'static str],
    /// Ordered operator steps for entering BOOTSEL mode.
    pub manual_steps: &'static [&'static str],
}

#[cfg(test)]
mod tests {
    use super::{Artifact, Bootsel, BuildProfile, FirmwareTarget, Probe, Uf2};

    #[test]
    fn build_profile_strings_are_stable() {
        assert_eq!(BuildProfile::Debug.as_str(), "debug");
        assert_eq!(BuildProfile::Release.as_str(), "release");
    }

    #[test]
    fn firmware_target_is_copyable_static_metadata() {
        const TARGET: FirmwareTarget = FirmwareTarget {
            id: "example",
            board_name: "Example Board",
            board_mcu: "Example MCU",
            artifact: Artifact {
                manifest_path: "firmware/example/Cargo.toml",
                package_name: "example-firmware",
                bin_name: "example-firmware",
                cargo_features: &["board-example"],
                cargo_no_default_features: true,
                cargo_target_dir: "target/example-firmware",
                target_triple: "thumbv8m.main-none-eabihf",
                default_profile: BuildProfile::Debug,
            },
            probe: Probe {
                chip: "ExampleChip",
            },
            uf2: Uf2 {
                family_name: "EXAMPLE",
                family_id: 0x1234_5678,
                absolute_block_location: Some(0x10ff_ff00),
            },
            bootsel: Bootsel {
                button_name: "BOOTSEL",
                volume_labels: &["EXAMPLE"],
                info_tokens: &["EXAMPLE"],
                manual_steps: &["Press BOOTSEL"],
            },
        };

        assert_eq!(TARGET.artifact.default_profile.as_str(), "debug");
        assert_eq!(TARGET.artifact.cargo_features, &["board-example"]);
        assert!(TARGET.artifact.cargo_no_default_features);
        assert_eq!(TARGET.artifact.cargo_target_dir, "target/example-firmware");
        assert_eq!(TARGET.uf2.family_id, 0x1234_5678);
        assert_eq!(TARGET.bootsel.manual_steps[0], "Press BOOTSEL");
    }
}
