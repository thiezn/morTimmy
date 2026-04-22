//! Configuration loading for live hardware integration tests.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Configuration used by ignored live-hardware integration tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HardwareTestConfig {
    /// Serial device used to reach the board under test.
    pub serial_device: String,
    /// Transport baud rate for the device under test.
    pub baud_rate: u32,
    /// Link timeout for live test operations.
    pub timeout_ms: u64,
    /// Whether the device is expected to support the firmware audio bridge.
    pub expect_audio_bridge: bool,
    /// Whether the device is expected to expose the Trellis module.
    pub expect_trellis: bool,
}

impl Default for HardwareTestConfig {
    fn default() -> Self {
        Self {
            serial_device: "/dev/ttyACM0".to_string(),
            baud_rate: 115_200,
            timeout_ms: 2_000,
            expect_audio_bridge: true,
            expect_trellis: true,
        }
    }
}

/// Load live-hardware test config from `MORTIMMY_HW_CONFIG` when present.
pub fn load_hardware_test_config() -> Result<Option<HardwareTestConfig>> {
    if let Ok(path) = std::env::var("MORTIMMY_HW_CONFIG") {
        return load_hardware_test_config_from_path(Path::new(&path)).map(Some);
    }

    Ok(None)
}

/// Load live-hardware test config from a specific TOML file path.
pub fn load_hardware_test_config_from_path(path: &Path) -> Result<HardwareTestConfig> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read hardware test config {}", path.display()))?;

    let config = toml::from_str(&contents)
        .with_context(|| format!("failed to parse hardware test config {}", path.display()))?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use std::path::PathBuf;

    use super::load_hardware_test_config_from_path;

    fn unique_temp_path(prefix: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{}_{}.toml", std::process::id(), nanos))
    }

    #[test]
    fn loads_config_from_path() {
        let path = unique_temp_path("mortimmy_hw_config_direct");
        std::fs::write(
            &path,
            "serial_device = \"/dev/ttyTEST0\"\nbaud_rate = 115200\ntimeout_ms = 1200\nexpect_audio_bridge = false\nexpect_trellis = true\n",
        )
        .unwrap();

        let config = load_hardware_test_config_from_path(&path).unwrap();
        assert_eq!(config.serial_device, "/dev/ttyTEST0");
        assert_eq!(config.timeout_ms, 1_200);
        assert!(config.expect_trellis);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn loads_config_from_path_with_overrides() {
        let path = unique_temp_path("mortimmy_hw_config");
        std::fs::write(
            &path,
            "serial_device = \"/dev/ttyTEST\"\nbaud_rate = 230400\ntimeout_ms = 5000\nexpect_audio_bridge = true\nexpect_trellis = false\n",
        )
        .unwrap();

        let config = load_hardware_test_config_from_path(&path).unwrap();
        assert_eq!(config.serial_device, "/dev/ttyTEST");
        assert_eq!(config.baud_rate, 230_400);
        assert!(!config.expect_trellis);

        let _ = std::fs::remove_file(path);
    }
}
