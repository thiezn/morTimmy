use serde::{Deserialize, Serialize};

use super::{
    AudioStatusTelemetry, BatteryTelemetry, DesiredStateTelemetry, RangeTelemetry,
    StatusTelemetry, TrellisPadTelemetry,
};

/// Telemetry messages sent from the firmware back to the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Telemetry {
    Status(StatusTelemetry),
    DesiredState(DesiredStateTelemetry),
    Range(RangeTelemetry),
    Battery(BatteryTelemetry),
    AudioStatus(AudioStatusTelemetry),
    TrellisPad(TrellisPadTelemetry),
    Pong,
}

impl Telemetry {
    /// Stable display name for logging and test assertions.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Status(_) => "status",
            Self::DesiredState(_) => "desired-state",
            Self::Range(_) => "range",
            Self::Battery(_) => "battery",
            Self::AudioStatus(_) => "audio-status",
            Self::TrellisPad(_) => "trellis-pad",
            Self::Pong => "pong",
        }
    }
}