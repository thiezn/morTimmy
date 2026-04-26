//! Firmware-to-host protocol telemetry surface.

pub mod audio;
pub mod battery;
pub mod capabilities;
pub mod desired_state;
pub mod drive;
pub mod range;
pub mod servo;
pub mod status;
pub mod trellis_pad;

use serde::{Deserialize, Serialize};

pub use self::audio::AudioStatusTelemetry;
pub use self::battery::BatteryTelemetry;
pub use self::capabilities::{ControllerCapabilities, ControllerRole};
pub use self::desired_state::DesiredStateTelemetry;
pub use self::drive::MotorStateTelemetry;
pub use self::range::RangeTelemetry;
pub use self::servo::ServoStateTelemetry;
pub use self::status::StatusTelemetry;
pub use self::trellis_pad::{PadEventKind, TRELLIS_PAD_COUNT, TrellisPadTelemetry};

/// Telemetry messages sent from the firmware back to the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Telemetry {
    Status(StatusTelemetry),
    DesiredState(DesiredStateTelemetry),
    Range(RangeTelemetry),
    Battery(BatteryTelemetry),
    AudioStatus(AudioStatusTelemetry),
    TrellisPad(TrellisPadTelemetry),
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
        }
    }
}
