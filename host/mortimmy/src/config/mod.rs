use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use cli_helpers::resolve_path;
use nexo_ws_schema::Platform;
use serde::{Deserialize, Serialize};

use crate::{
    audio::AudioConfig, camera::CameraConfig, serial::SerialConfig, telemetry::TelemetryConfig,
    websocket::WebsocketConfig,
};

pub use cli_helpers::LogLevel;
pub use cli_helpers::config::{load, load_or_create, save};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    pub health_check_interval_ms: u64,
    pub reconnect_interval_ms: u64,
    pub response_timeout_ms: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            health_check_interval_ms: 5_000,
            reconnect_interval_ms: 2_000,
            response_timeout_ms: 2_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NexoConfig {
    pub gateway_url: String,
    pub client_id: String,
    pub client_version: String,
    pub platform: Platform,
    pub device_id: String,
}

impl Default for NexoConfig {
    fn default() -> Self {
        Self {
            gateway_url: "ws://127.0.0.1:6969".to_string(),
            client_id: "cli".to_string(),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            platform: Platform::Mortimmy,
            device_id: "default_device".to_string(),
        }
    }
}

pub const fn nexo_platform_as_str(platform: Platform) -> &'static str {
    match platform {
        Platform::Macos => "macos",
        Platform::Ios => "ios",
        Platform::Linux => "linux",
        Platform::Windows => "windows",
        Platform::Mortimmy => "mortimmy",
    }
}

pub fn parse_nexo_platform(value: &str) -> std::result::Result<Platform, String> {
    match value.to_ascii_lowercase().as_str() {
        "macos" => Ok(Platform::Macos),
        "ios" => Ok(Platform::Ios),
        "linux" => Ok(Platform::Linux),
        "windows" => Ok(Platform::Windows),
        "mortimmy" => Ok(Platform::Mortimmy),
        _ => Err(format!(
            "unknown nexo platform `{value}`; expected one of macos, ios, linux, windows, mortimmy"
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: LogLevel,
    pub no_color: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            no_color: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub serial: SerialConfig,
    pub session: SessionConfig,
    pub nexo: NexoConfig,
    pub websocket: WebsocketConfig,
    pub telemetry: TelemetryConfig,
    pub audio: AudioConfig,
    pub camera: CameraConfig,
    pub logging: LoggingConfig,
}

pub fn resolve_config_path(path: Option<&Path>) -> Result<PathBuf> {
    match path {
        Some(path) => Ok(resolve_path(path)?),
        None => {
            if let Some(home_dir) = dirs::home_dir() {
                return Ok(home_dir.join(".mortimmy").join("config.toml"));
            }

            let cwd = std::env::current_dir().context("failed to determine current directory")?;
            Ok(cwd.join("config.toml"))
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use nexo_ws_schema::Platform;
    use std::path::PathBuf;

    use super::{AppConfig, LogLevel, load, load_or_create, save};
    use crate::camera::CameraBackendKind;

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos))
    }

    #[test]
    fn missing_file_uses_defaults() {
        let dir = unique_temp_dir("pi_daemon_config_defaults");
        let path = dir.join("config.toml");

        let config: AppConfig = load(&path).unwrap();
        assert_eq!(config, AppConfig::default());
    }

    #[test]
    fn load_or_create_bootstraps_file() {
        let dir = unique_temp_dir("pi_daemon_config_bootstrap");
        let path = dir.join("config.toml");

        let config: AppConfig = load_or_create(&path).unwrap();
        assert_eq!(config, AppConfig::default());
        assert!(path.exists());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn save_and_reload_roundtrip_with_sections() {
        let dir = unique_temp_dir("pi_daemon_config_roundtrip");
        let path = dir.join("config.toml");

        let mut config = AppConfig::default();
        config.serial.device_paths = vec!["/dev/ttyUSB0".to_string(), "/dev/ttyUSB1".to_string()];
        config.serial.baud_rate = 230_400;
        config.session.health_check_interval_ms = 1_500;
        config.session.reconnect_interval_ms = 750;
        config.session.response_timeout_ms = 3_000;
        config.nexo.gateway_url = "ws://localhost:7777".to_string();
        config.nexo.client_id = "cli-test".to_string();
        config.nexo.client_version = "9.9.9".to_string();
        config.nexo.platform = Platform::Linux;
        config.nexo.device_id = "device-test".to_string();
        config.websocket.bind_address = "0.0.0.0:9010".to_string();
        config.telemetry.publish_interval_ms = 50;
        config.telemetry.queue_capacity = 512;
        config.audio.enabled = true;
        config.audio.sample_rate_hz = 24_000;
        config.audio.chunk_samples = 240;
        config.camera.enabled = true;
        config.camera.backend = CameraBackendKind::Nokhwa;
        config.logging.level = LogLevel::Debug;
        config.logging.no_color = true;

        save(&config, &path).unwrap();
        let loaded = load(&path).unwrap();

        assert_eq!(config, loaded);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn load_rejects_legacy_single_serial_device_field() {
        let dir = unique_temp_dir("pi_daemon_config_legacy_serial");
        let path = dir.join("config.toml");

        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            &path,
            "[serial]\ndevice_path = \"/dev/ttyUSB9\"\nbaud_rate = 115200\n",
        )
        .unwrap();

        let error = load::<AppConfig>(&path).unwrap_err();

        assert!(format!("{error:#}").contains("unknown field `device_path`"));

        let _ = std::fs::remove_dir_all(dir);
    }
}
