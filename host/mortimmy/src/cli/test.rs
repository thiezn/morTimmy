use std::path::PathBuf;

use clap::Args;

use crate::brain::transport::TransportBackendKind;

#[derive(Debug, Args)]
pub struct TestCommand {
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
    #[arg(long = "reconnect-interval-ms")]
    pub reconnect_interval_ms: Option<u64>,
    #[arg(long = "connect-timeout-ms", default_value_t = 15_000)]
    pub connect_timeout_ms: u64,
    #[arg(long = "step-duration-ms", default_value_t = 600)]
    pub step_duration_ms: u64,
    #[arg(long = "drive-speed", default_value_t = 700)]
    pub drive_speed: u16,
}
