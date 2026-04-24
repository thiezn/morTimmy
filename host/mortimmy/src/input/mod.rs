//! Operator input backends that produce high-level brain commands.

mod gamepad;
mod registry;
mod scripted;
mod source;
mod websocket;

use clap::ValueEnum;

use anyhow::Result;

use crate::websocket::WebsocketServer;

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
    websocket: WebsocketServer,
) -> Result<ControllerRegistry> {
    let (usb_gamepad, bluetooth_gamepad) = self::gamepad::create_gamepad_inputs()?;
    let websocket = WebsocketControllerInput::new(websocket)?;

    Ok(ControllerRegistry::new(
        selection,
        vec![Box::new(usb_gamepad), Box::new(bluetooth_gamepad), Box::new(websocket)],
    ))
}

/// Selects which input backend the host brain uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum InputBackendKind {
    /// Run the local ratatui console with optional external controllers.
    #[default]
    Tui,
}
