use serde::{Deserialize, Serialize};

/// Runtime audio forwarding state reported by the firmware.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioStatusTelemetry {
    pub queued_chunks: u16,
    pub speaking: bool,
    pub underrun_count: u16,
}