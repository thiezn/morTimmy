//! Shared wire messages exchanged between the host and embedded controller.

pub mod controller;
pub mod commands;
pub mod host;
mod ids;
pub mod telemetry;
mod wire;

pub use self::controller::{
    AudioCommandResponse, ControllerEvent, ControllerMessage, ControllerResponse,
    ControllerResponsePayload, ReportMessage, ReportPayload, RequestOutcome,
};
pub use self::commands::{
    AUDIO_CHUNK_CAPACITY_SAMPLES, AudioChunkCommand, AudioEncoding, DesiredStateCommand,
    DriveCommand, ParameterKey, ParameterUpdate, ServoCommand, TrellisLedCommand,
};
pub use self::host::{ControlMessage, HostMessage, ReportConfig, RequestMessage, RequestPayload};
pub use self::ids::{ReportKind, RequestId};
pub use self::telemetry::{
    AudioStatusTelemetry, BatteryTelemetry, ControllerCapabilities, ControllerRole,
    ControllerStatus, ControlAppliedReport, ForwardRangeTelemetry, MotorStateTelemetry,
    PadEventKind, RangeSensorPosition, RangeTelemetry, ServoStateTelemetry,
    TRELLIS_PAD_COUNT, TrellisPadTelemetry,
};
pub use self::wire::WireMessage;
