use serde::{Deserialize, Serialize};

use super::{
    AudioChunkCommand, DesiredStateCommand, ParameterUpdate, TrellisLedCommand,
};

/// Command messages sent from the host to the firmware.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    SetDesiredState(DesiredStateCommand),
    SetParam(ParameterUpdate),
    PlayAudio(AudioChunkCommand),
    SetTrellisLeds(TrellisLedCommand),
    Ping,
}

impl Command {
    /// Stable display name for logging and test assertions.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::SetDesiredState(_) => "set-desired-state",
            Self::SetParam(_) => "set-param",
            Self::PlayAudio(_) => "play-audio",
            Self::SetTrellisLeds(_) => "set-trellis-leds",
            Self::Ping => "ping",
        }
    }
}