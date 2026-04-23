use mortimmy_core::Millimeters;
use serde::{Deserialize, Serialize};

/// Latest ultrasonic ranging sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeTelemetry {
    pub distance_mm: Millimeters,
    pub quality: u8,
}
