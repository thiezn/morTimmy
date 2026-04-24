use std::path::PathBuf;

use clap::{Args, builder::BoolishValueParser};
use nexo_ws_schema::Platform;

use crate::{
    audio::AudioBackendKind,
    camera::CameraBackendKind,
    config::{AppConfig, LogLevel, parse_nexo_platform},
};

#[derive(Debug, Args)]
pub struct ConfigCommand {
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,
    #[arg(long = "serial-device")]
    pub serial_device: Vec<String>,
    #[arg(long = "serial-baud-rate")]
    pub serial_baud_rate: Option<u32>,
    #[arg(long = "nexo-gateway-url")]
    pub nexo_gateway_url: Option<String>,
    #[arg(long = "nexo-client-id")]
    pub nexo_client_id: Option<String>,
    #[arg(long = "nexo-client-version")]
    pub nexo_client_version: Option<String>,
    #[arg(long = "nexo-platform", value_parser = parse_nexo_platform)]
    pub nexo_platform: Option<Platform>,
    #[arg(long = "nexo-device-id")]
    pub nexo_device_id: Option<String>,
    #[arg(long)]
    pub websocket_bind: Option<String>,
    #[arg(long = "telemetry-publish-interval-ms")]
    pub telemetry_publish_interval_ms: Option<u64>,
    #[arg(long = "telemetry-queue-capacity")]
    pub telemetry_queue_capacity: Option<usize>,
    #[arg(
        long = "audio-enabled",
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = BoolishValueParser::new()
    )]
    pub audio_enabled: Option<bool>,
    #[arg(long = "audio-backend", value_enum)]
    pub audio_backend: Option<AudioBackendKind>,
    #[arg(long = "audio-sample-rate-hz")]
    pub audio_sample_rate_hz: Option<u32>,
    #[arg(long = "audio-channels")]
    pub audio_channels: Option<u8>,
    #[arg(long = "audio-chunk-samples")]
    pub audio_chunk_samples: Option<usize>,
    #[arg(long = "audio-volume-percent")]
    pub audio_volume_percent: Option<u8>,
    #[arg(
        long = "camera-enabled",
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = BoolishValueParser::new()
    )]
    pub camera_enabled: Option<bool>,
    #[arg(long = "camera-backend", value_enum)]
    pub camera_backend: Option<CameraBackendKind>,
    #[arg(long = "camera-device-index")]
    pub camera_device_index: Option<u32>,
    #[arg(long = "camera-width")]
    pub camera_width: Option<u32>,
    #[arg(long = "camera-height")]
    pub camera_height: Option<u32>,
    #[arg(long = "camera-fps")]
    pub camera_fps: Option<u32>,
    #[arg(long, value_enum)]
    pub log_level: Option<LogLevel>,
    #[arg(
        long,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true",
        value_parser = BoolishValueParser::new()
    )]
    pub no_color: Option<bool>,
    #[arg(long, help = "Print the resulting config TOML after writing it")]
    pub print: bool,
}

impl ConfigCommand {
    pub fn apply_to(&self, config: &mut AppConfig) {
        if !self.serial_device.is_empty() {
            config.serial.device_paths = self.serial_device.clone();
        }
        if let Some(serial_baud_rate) = self.serial_baud_rate {
            config.serial.baud_rate = serial_baud_rate;
        }
        if let Some(nexo_gateway_url) = &self.nexo_gateway_url {
            config.nexo.gateway_url = nexo_gateway_url.clone();
        }
        if let Some(nexo_client_id) = &self.nexo_client_id {
            config.nexo.client_id = nexo_client_id.clone();
        }
        if let Some(nexo_client_version) = &self.nexo_client_version {
            config.nexo.client_version = nexo_client_version.clone();
        }
        if let Some(nexo_platform) = self.nexo_platform {
            config.nexo.platform = nexo_platform;
        }
        if let Some(nexo_device_id) = &self.nexo_device_id {
            config.nexo.device_id = nexo_device_id.clone();
        }
        if let Some(websocket_bind) = &self.websocket_bind {
            config.websocket.bind_address = websocket_bind.clone();
        }
        if let Some(telemetry_publish_interval_ms) = self.telemetry_publish_interval_ms {
            config.telemetry.publish_interval_ms = telemetry_publish_interval_ms;
        }
        if let Some(telemetry_queue_capacity) = self.telemetry_queue_capacity {
            config.telemetry.queue_capacity = telemetry_queue_capacity;
        }
        if let Some(audio_enabled) = self.audio_enabled {
            config.audio.enabled = audio_enabled;
        }
        if let Some(audio_backend) = self.audio_backend {
            config.audio.backend = audio_backend;
        }
        if let Some(audio_sample_rate_hz) = self.audio_sample_rate_hz {
            config.audio.sample_rate_hz = audio_sample_rate_hz;
        }
        if let Some(audio_channels) = self.audio_channels {
            config.audio.channels = audio_channels;
        }
        if let Some(audio_chunk_samples) = self.audio_chunk_samples {
            config.audio.chunk_samples = audio_chunk_samples;
        }
        if let Some(audio_volume_percent) = self.audio_volume_percent {
            config.audio.volume_percent = audio_volume_percent;
        }
        if let Some(camera_enabled) = self.camera_enabled {
            config.camera.enabled = camera_enabled;
        }
        if let Some(camera_backend) = self.camera_backend {
            config.camera.backend = camera_backend;
        }
        if let Some(camera_device_index) = self.camera_device_index {
            config.camera.device_index = camera_device_index;
        }
        if let Some(camera_width) = self.camera_width {
            config.camera.width = camera_width;
        }
        if let Some(camera_height) = self.camera_height {
            config.camera.height = camera_height;
        }
        if let Some(camera_fps) = self.camera_fps {
            config.camera.fps = camera_fps;
        }
        if let Some(log_level) = self.log_level {
            config.logging.level = log_level;
        }
        if let Some(no_color) = self.no_color {
            config.logging.no_color = no_color;
        }
    }
}
