use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::brain::transport::TransportBackendKind;

/// `mortimmy test` command line, including shared options and the selected target.
#[derive(Debug, Clone, Args)]
pub struct TestCommand {
    #[command(flatten)]
    pub options: TestOptions,
    #[command(subcommand)]
    pub target: Option<TestTarget>,
}

impl TestCommand {
    /// Return the selected target, defaulting to `All` when no subcommand is provided.
    pub fn selected_target(&self) -> TestTarget {
        self.target.clone().unwrap_or(TestTarget::All)
    }
}

/// Shared options applied to every `mortimmy test` subcommand.
#[derive(Debug, Clone, Args)]
pub struct TestOptions {
    #[arg(long, value_name = "PATH", global = true)]
    pub config: Option<PathBuf>,
    #[arg(
        long = "transport-backend",
        value_enum,
        default_value_t = TransportBackendKind::Serial,
        global = true
    )]
    pub transport_backend: TransportBackendKind,
    #[arg(long = "serial-device", global = true)]
    pub serial_device: Vec<String>,
    #[arg(long = "serial-baud-rate", global = true)]
    pub serial_baud_rate: Option<u32>,
    #[arg(long = "response-timeout-ms", global = true)]
    pub response_timeout_ms: Option<u64>,
    #[arg(long = "reconnect-interval-ms", global = true)]
    pub reconnect_interval_ms: Option<u64>,
    #[arg(long = "connect-timeout-ms", default_value_t = 15_000, global = true)]
    pub connect_timeout_ms: u64,
    #[arg(long = "step-duration-ms", default_value_t = 600, global = true)]
    pub step_duration_ms: u64,
    #[arg(long = "drive-speed", default_value_t = 700, global = true)]
    pub drive_speed: u16,
    #[arg(long = "motion-duration-ms", default_value_t = 1_500, global = true)]
    pub motion_duration_ms: u64,
    #[arg(long = "pause-duration-ms", default_value_t = 750, global = true)]
    pub pause_duration_ms: u64,
    #[arg(long = "servo-hold-ms", default_value_t = 900, global = true)]
    pub servo_hold_ms: u64,
    #[arg(long = "servo-step-ticks", default_value_t = 24, global = true)]
    pub servo_step_ticks: u16,
    #[arg(long = "sensor-listen-ms", default_value_t = 3_000, global = true)]
    pub sensor_listen_ms: u64,
    #[arg(long = "report-interval-ms", default_value_t = 250, global = true)]
    pub report_interval_ms: u32,
    #[arg(long = "audio-duration-ms", default_value_t = 500, global = true)]
    pub audio_duration_ms: u64,
}

/// Non-TUI validation flows exposed by `mortimmy test`.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum TestTarget {
    /// Run the full available protocol test suite, skipping unsupported capability-specific checks.
    All,
    /// Query and print controller identity and health responses.
    Status,
    /// Exercise latest-wins drive control and verify control-applied reports.
    Drive,
    /// Exercise servo control without going through the TUI.
    Servo,
    /// Listen for incoming range and battery reports.
    Sensors,
    /// Send a short audio waveform through the firmware bridge.
    Audio,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::TestTarget;
    use crate::cli::base::{Cli, Command};

    #[test]
    fn defaults_test_target_to_all() {
        let cli = Cli::parse_from(["mortimmy-pi-daemon", "test"]);

        let Command::Test(command) = cli.command else {
            panic!("expected test command");
        };

        assert_eq!(command.selected_target(), TestTarget::All);
    }

    #[test]
    fn parses_drive_test_subcommand() {
        let cli = Cli::parse_from(["mortimmy-pi-daemon", "test", "drive"]);

        let Command::Test(command) = cli.command else {
            panic!("expected test command");
        };

        assert_eq!(command.selected_target(), TestTarget::Drive);
    }
}
