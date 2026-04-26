use mortimmy_core::Mode;

/// High-level operator intent produced by local or remote input devices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrainCommand {
    Quit,
    Stop,
    SetMode(Mode),
    Chat(String),
    #[allow(dead_code)]
    Servo {
        pan: u16,
        tilt: u16,
    },
}
