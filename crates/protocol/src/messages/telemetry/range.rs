use mortimmy_core::Millimeters;
use serde::{Deserialize, Serialize};

/// Stable identity for each forward-facing ultrasonic sensor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RangeSensorPosition {
    ForwardLeft,
    ForwardRight,
}

/// Latest known samples for the forward ultrasonic pair.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardRangeTelemetry {
    pub forward_left: Option<RangeTelemetry>,
    pub forward_right: Option<RangeTelemetry>,
}

/// Latest ultrasonic ranging sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeTelemetry {
    pub sensor: RangeSensorPosition,
    pub distance_mm: Millimeters,
    pub quality: u8,
}
