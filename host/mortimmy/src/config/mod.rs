use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::{
    audio::AudioConfig, camera::CameraConfig, serial::SerialConfig, telemetry::TelemetryConfig,
    websocket::WebsocketConfig,
};

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

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
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
    pub websocket: WebsocketConfig,
    pub telemetry: TelemetryConfig,
    pub audio: AudioConfig,
    pub camera: CameraConfig,
    pub logging: LoggingConfig,
}

pub fn load(path: &Path) -> Result<AppConfig> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }

    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;

    let config = toml::from_str(&contents)
        .with_context(|| format!("failed to parse config file {}", path.display()))?;

    Ok(config)
}

pub fn load_or_create(path: &Path) -> Result<AppConfig> {
    if path.exists() {
        return load(path);
    }

    let config = AppConfig::default();
    save(&config, path)?;
    Ok(config)
}

pub fn save(config: &AppConfig, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    }

    let serialized = toml::to_string_pretty(config).context("failed to serialize config")?;
    fs::write(path, serialized)
        .with_context(|| format!("failed to write config file {}", path.display()))?;

    Ok(())
}

pub fn resolve_config_path(path: Option<&Path>) -> Result<PathBuf> {
    match path {
        Some(path) => {
            if let Some(path_str) = path.to_str()
                && let Some(rest) = path_str.strip_prefix("~/")
            {
                let home_dir = dirs::home_dir().context("failed to determine home directory")?;
                return Ok(home_dir.join(rest));
            }

            if path == Path::new("~") {
                return dirs::home_dir().context("failed to determine home directory");
            }

            if path.is_absolute() {
                return Ok(path.to_path_buf());
            }

            let cwd = std::env::current_dir().context("failed to determine current directory")?;
            Ok(cwd.join(path))
        }
        None => {
            if let Some(config_dir) = dirs::home_dir() {
                return Ok(config_dir.join(".mortimmy").join("config.toml"));
            }

            let cwd = std::env::current_dir().context("failed to determine current directory")?;
            Ok(cwd.join("config.toml"))
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

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

        let config = load(&path).unwrap();
        assert_eq!(config, AppConfig::default());
    }

    #[test]
    fn load_or_create_bootstraps_file() {
        let dir = unique_temp_dir("pi_daemon_config_bootstrap");
        let path = dir.join("config.toml");

        let config = load_or_create(&path).unwrap();
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

        let error = load(&path).unwrap_err();

        assert!(format!("{error:#}").contains("unknown field `device_path`"));

        let _ = std::fs::remove_dir_all(dir);
    }
}
