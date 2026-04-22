//! Shared wire messages exchanged between the host and embedded controller.

mod audio;
mod battery;
mod command;
mod desired_state;
mod drive;
mod parameter;
mod range;
mod servo;
mod status;
mod telemetry;
mod trellis_led;
mod trellis_pad;
mod wire;

pub use self::audio::{
    AUDIO_CHUNK_CAPACITY_SAMPLES, AudioChunkCommand, AudioEncoding, AudioStatusTelemetry,
};
pub use self::battery::BatteryTelemetry;
pub use self::command::Command;
pub use self::desired_state::{DesiredStateCommand, DesiredStateTelemetry};
pub use self::drive::{DriveCommand, MotorStateTelemetry};
pub use self::parameter::{ParameterKey, ParameterUpdate};
pub use self::range::RangeTelemetry;
pub use self::servo::{ServoCommand, ServoStateTelemetry};
pub use self::status::StatusTelemetry;
pub use self::telemetry::Telemetry;
pub use self::trellis_led::TrellisLedCommand;
pub use self::trellis_pad::{PadEventKind, TRELLIS_PAD_COUNT, TrellisPadTelemetry};
pub use self::wire::WireMessage;