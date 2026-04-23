use serde::{Deserialize, Serialize};

/// 16-bit Trellis LED mask where each bit maps to a single pad LED.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrellisLedCommand {
    pub led_mask: u16,
}