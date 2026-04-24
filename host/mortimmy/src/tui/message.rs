use mortimmy_core::Mode;

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
    Log(LogLevel, String),
    ShowHelp(bool),
    ControllerConnected(ControllerInfo),
    ControllerDisconnected(ControllerInfo),
}
