use mortimmy_core::CoreError;
use serde::{Deserialize, Serialize};

use super::{
    ids::RequestId,
    telemetry::{
        AudioStatusTelemetry, BatteryTelemetry, ControllerStatus, ControlAppliedReport,
        RangeTelemetry, TrellisPadTelemetry,
    },
};

/// Messages initiated by the controller side of the link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControllerMessage {
    /// Correlated reply to a host request.
    Response(ControllerResponse),
    /// Asynchronous controller data with its own cadence.
    Report(ReportMessage),
    /// Immediate controller-side event.
    Event(ControllerEvent),
}

impl ControllerMessage {
    /// Stable display name for logging and test assertions.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Response(response) => response.kind(),
            Self::Report(report) => report.kind(),
            Self::Event(event) => event.kind(),
        }
    }
}

/// Correlated controller response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerResponse {
    pub request_id: RequestId,
    pub payload: ControllerResponsePayload,
}

impl ControllerResponse {
    /// Return the stable kind string for this response payload.
    pub const fn kind(&self) -> &'static str {
        self.payload.kind()
    }
}

/// Response payloads sent by the controller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControllerResponsePayload {
    /// Snapshot of controller identity, health, and capabilities.
    ControllerStatus(ControllerStatus),
    /// Outcome of a parameter update request.
    Parameter(RequestOutcome),
    /// Outcome of an audio chunk request plus the latest queue state.
    Audio(AudioCommandResponse),
    /// Outcome of a Trellis LED update request.
    TrellisLeds(RequestOutcome),
    /// Outcome of a report cadence configuration request.
    ReportConfig(RequestOutcome),
}

impl ControllerResponsePayload {
    /// Return the stable kind string for this response payload.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::ControllerStatus(_) => "controller-status",
            Self::Parameter(_) => "parameter-response",
            Self::Audio(_) => "audio-response",
            Self::TrellisLeds(_) => "trellis-leds-response",
            Self::ReportConfig(_) => "report-config-response",
        }
    }
}

/// Generic outcome for a one-shot request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestOutcome {
    pub error: Option<CoreError>,
}

impl RequestOutcome {
    /// Return a successful outcome with no error.
    pub const fn ok() -> Self {
        Self { error: None }
    }

    /// Return a failed outcome carrying `error`.
    pub const fn from_error(error: CoreError) -> Self {
        Self { error: Some(error) }
    }
}

/// Audio request response carrying the latest queue state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioCommandResponse {
    pub status: AudioStatusTelemetry,
    pub error: Option<CoreError>,
}

/// Controller-originated reports with independent cadence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportPayload {
    /// Applied desired-control state for the latest host generation.
    ControlApplied(ControlAppliedReport),
    /// Range measurement from one ultrasonic sensor.
    Range(RangeTelemetry),
    /// Battery monitor reading.
    Battery(BatteryTelemetry),
    /// Audio queue state report.
    AudioStatus(AudioStatusTelemetry),
}

impl ReportPayload {
    /// Return the stable kind string for this report payload.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::ControlApplied(_) => "control-applied",
            Self::Range(_) => "range",
            Self::Battery(_) => "battery",
            Self::AudioStatus(_) => "audio-status",
        }
    }
}

/// Report wrapper used on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportMessage {
    pub payload: ReportPayload,
}

impl ReportMessage {
    /// Return the stable kind string for the wrapped report payload.
    pub const fn kind(&self) -> &'static str {
        self.payload.kind()
    }
}

/// Controller-originated events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControllerEvent {
    /// Trellis keypad input event.
    TrellisPad(TrellisPadTelemetry),
}

impl ControllerEvent {
    /// Return the stable kind string for this event payload.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::TrellisPad(_) => "trellis-pad",
        }
    }
}
