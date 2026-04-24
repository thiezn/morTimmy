//! Operator input backends that produce high-level brain commands.

mod gamepad;
mod keyboard;
mod registry;
mod scripted;
mod source;
mod websocket;

use clap::ValueEnum;

use anyhow::Result;

use crate::websocket::WebsocketServer;

pub use self::keyboard::{KeyboardDriveStyle, KeyboardInput};
pub use self::registry::{
    ControllerBackend, ControllerLifecycleEvent, ControllerRegistry, ControllerSelection,
    RoutedInputEvent, SourcedInputEvent,
};
#[allow(unused_imports)]
pub use self::scripted::ScriptedInput;
pub use self::source::{
    CommandInputSource, ControlState, ControllerId, ControllerInfo, ControllerKind, DriveIntent,
    InputEvent, InputWarning,
};
pub use self::websocket::WebsocketControllerInput;

pub fn default_controller_registry(
    selection: ControllerSelection,
    keyboard_drive_style: KeyboardDriveStyle,
    websocket: WebsocketServer,
) -> Result<ControllerRegistry> {
    let (usb_gamepad, bluetooth_gamepad) = self::gamepad::create_gamepad_inputs()?;
    let websocket = WebsocketControllerInput::new(websocket)?;

    Ok(ControllerRegistry::new(
        selection,
        vec![
            Box::new(KeyboardInput::with_drive_style(keyboard_drive_style)),
            Box::new(usb_gamepad),
            Box::new(bluetooth_gamepad),
            Box::new(websocket),
        ],
    ))
}

/// Selects which input backend the host brain uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum InputBackendKind {
    /// Read commands from stdin entered on the keyboard.
    #[default]
    Keyboard,
}
