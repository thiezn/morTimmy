use serde::{Deserialize, Serialize};

use super::{controller::ControllerMessage, host::HostMessage};

/// Top-level bidirectional protocol message.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireMessage {
    /// Host-originated message traveling toward the controller.
    Host(HostMessage),
    /// Controller-originated message traveling toward the host.
    Controller(ControllerMessage),
}

impl WireMessage {
    /// Stable display name for logging and test assertions.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Host(message) => message.kind(),
            Self::Controller(message) => message.kind(),
        }
    }
}
