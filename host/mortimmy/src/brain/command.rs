use mortimmy_core::Mode;

/// High-level operator intent produced by local or remote input devices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrainCommand {
    Quit,
    Ping,
    Stop,
    SetMode(Mode),
    #[allow(dead_code)]
    Servo { pan: u16, tilt: u16 },
}
