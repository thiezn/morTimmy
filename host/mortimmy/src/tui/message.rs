use mortimmy_core::Mode;
use mortimmy_protocol::messages::telemetry::RangeTelemetry;

use crate::{
    config::LogLevel,
    input::{ControlState, ControllerInfo},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    InsertChar(char),
    Backspace,
    DeleteForward,
    MoveCursorLeft,
    MoveCursorRight,
    MoveCursorStart,
    MoveCursorEnd,
    SubmitInput,
    SelectNextCompletion,
    SelectPreviousCompletion,
    ApplySelectedCompletion,
    ScrollActivityUp,
    ScrollActivityDown,
    CopyAllActivity,
    CopyLastActivity,
    SetConnectionStatus(String),
    SetControlState(ControlState),
    SetDesiredMode(Mode),
    SetDistance(Option<RangeTelemetry>),
    Log(LogLevel, String),
    ShowHelp(bool),
    ControllerConnected(ControllerInfo),
    ControllerDisconnected(ControllerInfo),
}
