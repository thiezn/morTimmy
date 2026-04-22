use std::fmt;
use std::time::Duration;

use anyhow::Result;

use crate::brain::BrainCommand;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DriveIntent {
    pub forward: i16,
    pub turn: i16,
    pub speed: u16,
}

impl DriveIntent {
    pub const AXIS_MAX: i16 = 1_000;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlState {
    pub drive: Option<DriveIntent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputWarning {
    UnknownKeyboardCommand(char),
}

impl fmt::Display for InputWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKeyboardCommand(character) => {
                write!(f, "unknown keyboard command: {character}")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEvent {
    Command(BrainCommand),
    Control(ControlState),
    Warning(InputWarning),
}

impl From<BrainCommand> for InputEvent {
    fn from(value: BrainCommand) -> Self {
        Self::Command(value)
    }
}

impl From<ControlState> for InputEvent {
    fn from(value: ControlState) -> Self {
        Self::Control(value)
    }
}

impl From<InputWarning> for InputEvent {
    fn from(value: InputWarning) -> Self {
        Self::Warning(value)
    }
}

/// Generic interface implemented by input backends that drive the robot brain.
pub trait CommandInputSource {
    /// Return human-readable usage information for this input backend.
    fn instructions(&self) -> Option<&'static str> {
        None
    }

    /// Block until the next high-level input event is available.
    fn next_event(&mut self) -> Result<InputEvent>;

    /// Wait up to `timeout` for the next high-level input event.
    fn poll_event(&mut self, _timeout: Duration) -> Result<Option<InputEvent>> {
        self.next_event().map(Some)
    }

    /// Pause operator input while the transport is disconnected.
    fn suspend(&mut self) -> Result<()> {
        Ok(())
    }

    /// Resume operator input after the transport reconnects.
    fn resume(&mut self) -> Result<()> {
        Ok(())
    }

    /// Extend any in-progress continuous control hold after a blocking command roundtrip.
    fn extend_active_control(&mut self, _duration: Duration) -> Result<()> {
        Ok(())
    }
}
