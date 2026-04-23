use serde::{Deserialize, Serialize};

/// Number of pads on the Trellis 4x4 grid.
pub const TRELLIS_PAD_COUNT: usize = 16;

/// Trellis pad transition kinds forwarded to the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PadEventKind {
    Pressed,
    Released,
}

/// Single Trellis pad transition emitted by the firmware.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrellisPadTelemetry {
    pub pad_index: u8,
    pub event: PadEventKind,
}
