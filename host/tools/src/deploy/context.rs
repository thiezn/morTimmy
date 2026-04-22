//! Workspace and target metadata helpers for deployment workflows.

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use mortimmy_deploy::{BuildProfile as DefaultBuildProfile, FirmwareTarget};

/// Resolved mortimmy workspace root.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// Discover the workspace root by walking upward from the current working directory.
    pub fn discover() -> Result<Self> {
        let start_dir = env::current_dir().context("failed to read current working directory")?;
        let mut candidate = start_dir.clone();

        loop {
            let manifest_path = candidate.join("Cargo.toml");
            if manifest_path.is_file() && is_workspace_manifest(&manifest_path)? {
                return Ok(Self { root: candidate });
            }

            if !candidate.pop() {
                break;
            }
        }

        bail!(
            "failed to locate the mortimmy workspace root from {}; run mortimmy-tools from inside the repository",
            start_dir.display()
        )
    }

    /// Absolute path to the workspace root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a repository-relative path inside the workspace.
    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// Resolve a user-supplied path relative to the current shell working directory.
    pub fn resolve_user_path(&self, path: &Path) -> Result<PathBuf> {
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(env::current_dir()
                .context("failed to read current working directory")?
                .join(path))
        }
    }
}

fn is_workspace_manifest(path: &Path) -> Result<bool> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read manifest {}", path.display()))?;
    Ok(contents.contains("[workspace]"))
}

/// Firmware targets exported from firmware crates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FirmwareTargetId {
    Rp2350,
}

impl FirmwareTargetId {
    /// Resolve the selected target to its exported deploy metadata.
    pub fn metadata(self) -> &'static FirmwareTarget {
        match self {
            Self::Rp2350 => &mortimmy_rp2350::DEPLOY_TARGET,
        }
    }
}

/// Cargo profile selection used to build host and firmware artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoProfile {
    name: String,
}

impl CargoProfile {
    /// Use the explicit CLI profile override or fall back to firmware metadata defaults.
    pub fn from_cli_or_default(override_profile: Option<&String>, default: DefaultBuildProfile) -> Self {
        match override_profile {
            Some(profile) => Self::from_name(profile.clone()),
            None => Self::from_name(default.as_str().to_owned()),
        }
    }

    /// Construct a profile from a raw Cargo profile name.
    pub fn from_name(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Cargo CLI arguments required to select this profile.
    pub fn cargo_args(&self) -> Vec<String> {
        match self.name.as_str() {
            "debug" => Vec::new(),
            "release" => vec!["--release".to_owned()],
            _ => vec!["--profile".to_owned(), self.name.clone()],
        }
    }

    /// Directory name used under `target/` for this profile.
    pub fn artifact_dir_name(&self) -> &str {
        &self.name
    }

    /// Human-readable profile label.
    pub fn display_name(&self) -> &str {
        &self.name
    }
}

/// Resolve the absolute ELF path for a firmware target and profile.
pub fn firmware_elf_path(workspace: &Workspace, target: &FirmwareTarget, profile: &CargoProfile) -> PathBuf {
    workspace
        .root()
        .join("target")
        .join(target.artifact.target_triple)
        .join(profile.artifact_dir_name())
        .join(target.artifact.bin_name)
}

/// Resolve the default absolute UF2 path for a firmware target and profile.
pub fn default_firmware_uf2_path(
    workspace: &Workspace,
    target: &FirmwareTarget,
    profile: &CargoProfile,
) -> PathBuf {
    workspace
        .root()
        .join("target")
        .join(target.artifact.target_triple)
        .join(profile.artifact_dir_name())
        .join(format!("{}.uf2", target.artifact.bin_name))
}

/// Resolve the absolute host binary path for a build profile.
pub fn host_artifact_path(workspace: &Workspace, profile: &CargoProfile, bin_name: &str) -> PathBuf {
    workspace
        .root()
        .join("target")
        .join(profile.artifact_dir_name())
        .join(bin_name)
}
