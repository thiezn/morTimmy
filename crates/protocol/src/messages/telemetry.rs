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

pub use self::audio::AudioStatusTelemetry;
pub use self::battery::BatteryTelemetry;
pub use self::capabilities::{ControllerCapabilities, ControllerRole};
pub use self::desired_state::ControlAppliedReport;
pub use self::drive::MotorStateTelemetry;
pub use self::range::{ForwardRangeTelemetry, RangeSensorPosition, RangeTelemetry};
pub use self::servo::ServoStateTelemetry;
pub use self::status::ControllerStatus;
pub use self::trellis_pad::{PadEventKind, TRELLIS_PAD_COUNT, TrellisPadTelemetry};
