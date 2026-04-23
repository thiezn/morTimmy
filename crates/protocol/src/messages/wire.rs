use serde::{Deserialize, Serialize};

use super::{command::Command, telemetry::Telemetry};

/// Top-level bidirectional protocol message.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireMessage {
    Command(Command),
    Telemetry(Telemetry),
}

impl WireMessage {
    /// Stable display name for logging and test assertions.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Command(command) => command.kind(),
            Self::Telemetry(telemetry) => telemetry.kind(),
        }
    }
}
