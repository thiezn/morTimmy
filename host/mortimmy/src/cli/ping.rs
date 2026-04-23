use std::path::PathBuf;

use clap::Args;

use crate::{brain::transport::TransportBackendKind, config::AppConfig};

#[derive(Debug, Args)]
pub struct PingCommand {
    #[arg(long, value_name = "PATH")]
    pub config: Option<PathBuf>,
    #[arg(long = "transport-backend", value_enum, default_value_t = TransportBackendKind::Serial)]
    pub transport_backend: TransportBackendKind,
    #[arg(long = "serial-device")]
    pub serial_device: Vec<String>,
    #[arg(long = "serial-baud-rate")]
    pub serial_baud_rate: Option<u32>,
    #[arg(long = "response-timeout-ms")]
    pub response_timeout_ms: Option<u64>,
}

impl PingCommand {
    pub fn merge_config(self, mut config: AppConfig) -> AppConfig {
        if !self.serial_device.is_empty() {
            config.serial.device_paths = self.serial_device;
        }
        if let Some(serial_baud_rate) = self.serial_baud_rate {
            config.serial.baud_rate = serial_baud_rate;
        }
        if let Some(response_timeout_ms) = self.response_timeout_ms {
            config.session.response_timeout_ms = response_timeout_ms;
        }

        config
    }
}
