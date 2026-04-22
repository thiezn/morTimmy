#[cfg(feature = "camera-nokhwa")]
use nokhwa as _;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CameraBackendKind {
    #[default]
    Disabled,
    Nokhwa,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraConfig {
    pub enabled: bool,
    pub backend: CameraBackendKind,
    pub device_index: u32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: CameraBackendKind::Disabled,
            device_index: 0,
            width: 640,
            height: 480,
            fps: 30,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CameraSubsystem {
    pub config: CameraConfig,
    #[cfg_attr(not(test), allow(dead_code))]
    active_backend: &'static str,
}

impl CameraSubsystem {
    pub fn from_config(config: CameraConfig) -> Self {
        let active_backend = match (config.enabled, config.backend) {
            (false, _) | (_, CameraBackendKind::Disabled) => "disabled",
            (true, CameraBackendKind::Nokhwa) if cfg!(feature = "camera-nokhwa") => "nokhwa",
            (true, CameraBackendKind::Nokhwa) => "nokhwa-unavailable",
        };

        Self {
            config,
            active_backend,
        }
    }

    pub fn config(&self) -> &CameraConfig {
        &self.config
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub const fn active_backend_name(&self) -> &'static str {
        self.active_backend
    }
}

#[cfg(test)]
mod tests {
    use super::{CameraBackendKind, CameraConfig, CameraSubsystem};

    #[test]
    fn disabled_camera_stays_disabled() {
        let subsystem = CameraSubsystem::from_config(CameraConfig::default());
        assert_eq!(subsystem.active_backend_name(), "disabled");
    }

    #[test]
    fn nokhwa_without_feature_is_reported() {
        let subsystem = CameraSubsystem::from_config(CameraConfig {
            enabled: true,
            backend: CameraBackendKind::Nokhwa,
            ..CameraConfig::default()
        });

        assert_eq!(subsystem.active_backend_name(), "nokhwa-unavailable");
    }
}
