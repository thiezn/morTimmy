//! Operator input backends that produce high-level brain commands.

mod keyboard;
mod scripted;
mod source;

use clap::ValueEnum;

pub use self::keyboard::KeyboardInput;
#[allow(unused_imports)]
pub use self::scripted::ScriptedInput;
pub use self::source::{CommandInputSource, ControlState, DriveIntent, InputEvent};

/// Selects which input backend the host brain uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum InputBackendKind {
    /// Read commands from stdin entered on the keyboard.
    #[default]
    Keyboard,
}
