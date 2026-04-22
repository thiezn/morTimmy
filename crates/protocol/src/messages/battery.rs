use serde::{Deserialize, Serialize};

/// Latest battery monitor sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryTelemetry {
    pub millivolts: u16,
}