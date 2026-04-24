use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use anyhow::Result;
use clap::ValueEnum;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, ValueEnum)]
pub enum ControllerKind {
    Keyboard,
    GamepadUsb,
    GamepadBluetooth,
    Websocket,
}

impl ControllerKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Keyboard => "keyboard",
            Self::GamepadUsb => "gamepad-usb",
            Self::GamepadBluetooth => "gamepad-bluetooth",
            Self::Websocket => "websocket",
        }
    }
}

impl fmt::Display for ControllerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ControllerKind {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "keyboard" => Ok(Self::Keyboard),
            "gamepad-usb" => Ok(Self::GamepadUsb),
            "gamepad-bluetooth" => Ok(Self::GamepadBluetooth),
            "websocket" => Ok(Self::Websocket),
            _ => anyhow::bail!(
                "unknown controller kind `{value}`; expected one of keyboard, gamepad-usb, gamepad-bluetooth, websocket"
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ControllerId {
    pub kind: ControllerKind,
    pub instance: String,
}

impl ControllerId {
    pub fn new(kind: ControllerKind, instance: impl Into<String>) -> Self {
        Self {
            kind,
            instance: instance.into(),
        }
    }
}

impl fmt::Display for ControllerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind, self.instance)
    }
}

impl FromStr for ControllerId {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        let (kind, instance) = value.split_once(':').ok_or_else(|| {
            anyhow::anyhow!("invalid controller id `{value}`; expected <kind>:<instance>")
        })?;

        if instance.is_empty() {
            anyhow::bail!("invalid controller id `{value}`; instance must not be empty");
        }

        Ok(Self::new(
            <ControllerKind as FromStr>::from_str(kind)?,
            instance,
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControllerInfo {
    pub id: ControllerId,
    pub display_name: String,
}

impl ControllerInfo {
    pub fn new(id: ControllerId, display_name: impl Into<String>) -> Self {
        Self {
            id,
            display_name: display_name.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputWarning {
    UnknownKeyboardCommand(char),
    Status(Cow<'static, str>),
}

impl fmt::Display for InputWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKeyboardCommand(character) => {
                write!(f, "unknown keyboard command: {character}")
            }
            Self::Status(message) => f.write_str(message),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputEvent {
    Command(BrainCommand),
    Control(ControlState),
    Warning(InputWarning),
    Prompt(Option<String>),
    ControllerConnected(ControllerInfo),
    ControllerDisconnected(ControllerInfo),
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

impl From<ControllerInfo> for InputEvent {
    fn from(value: ControllerInfo) -> Self {
        Self::ControllerConnected(value)
    }
}

/// Generic interface implemented by input backends that drive the robot brain.
pub trait CommandInputSource {
    /// Return human-readable usage information for this input backend.
    fn instructions(&self) -> Option<Cow<'static, str>> {
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
