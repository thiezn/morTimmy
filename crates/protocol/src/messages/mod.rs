//! Shared wire messages exchanged between the host and embedded controller.

pub mod command;
pub mod commands;
pub mod telemetry;
mod wire;

pub use self::command::Command;
pub use self::commands::{
    AUDIO_CHUNK_CAPACITY_SAMPLES, AudioChunkCommand, AudioEncoding, DesiredStateCommand,
    DriveCommand, ParameterKey, ParameterUpdate, ServoCommand, TrellisLedCommand,
};
pub use self::telemetry::{
    AudioStatusTelemetry, BatteryTelemetry, ControllerCapabilities, ControllerRole,
    DesiredStateTelemetry, MotorStateTelemetry, PadEventKind, RangeTelemetry, ServoStateTelemetry,
    StatusTelemetry, TRELLIS_PAD_COUNT, Telemetry, TrellisPadTelemetry,
};
pub use self::wire::WireMessage;
