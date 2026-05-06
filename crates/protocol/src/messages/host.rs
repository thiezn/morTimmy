use serde::{Deserialize, Serialize};

use super::{
    commands::{AudioChunkCommand, DesiredStateCommand, ParameterUpdate, TrellisLedCommand},
    ids::{ReportKind, RequestId},
};

/// Messages initiated by the host side of the controller link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostMessage {
    /// Latest-wins desired control snapshot.
    Control(ControlMessage),
    /// Correlated one-shot request.
    Request(RequestMessage),
}

impl HostMessage {
    /// Stable display name for logging and test assertions.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Control(_) => "control",
            Self::Request(request) => request.kind(),
        }
    }
}

/// Latest-wins desired control state from the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlMessage {
    pub generation: u32,
    pub desired_state: DesiredStateCommand,
}

impl ControlMessage {
    /// Create a control message for `generation` and `desired_state`.
    pub const fn new(generation: u32, desired_state: DesiredStateCommand) -> Self {
        Self {
            generation,
            desired_state,
        }
    }
}

/// Correlated host request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestMessage {
    pub request_id: RequestId,
    pub payload: RequestPayload,
}

impl RequestMessage {
    /// Return the stable kind string for the wrapped request payload.
    pub const fn kind(&self) -> &'static str {
        self.payload.kind()
    }
}

/// One-shot host requests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequestPayload {
    /// Request a controller identity and health snapshot.
    GetControllerStatus,
    /// Update one controller parameter.
    SetParam(ParameterUpdate),
    /// Forward one chunk of audio samples.
    PlayAudio(AudioChunkCommand),
    /// Update the Trellis LED mask.
    SetTrellisLeds(TrellisLedCommand),
    /// Configure cadence for one controller-originated report class.
    ConfigureReports(ReportConfig),
}

impl RequestPayload {
    /// Return the stable kind string for this request payload.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::GetControllerStatus => "get-controller-status",
            Self::SetParam(_) => "set-param",
            Self::PlayAudio(_) => "play-audio",
            Self::SetTrellisLeds(_) => "set-trellis-leds",
            Self::ConfigureReports(_) => "configure-reports",
        }
    }
}

/// Per-report cadence configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportConfig {
    pub report: ReportKind,
    pub min_interval_ms: u32,
    pub emit_on_change: bool,
}
