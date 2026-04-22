use mortimmy_core::ServoTicks;
use serde::{Deserialize, Serialize};

/// Pan and tilt request for the active servo pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServoCommand {
    pub pan: ServoTicks,
    pub tilt: ServoTicks,
}

/// Current servo position state after host clamping and firmware safety checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServoStateTelemetry {
    pub pan: ServoTicks,
    pub tilt: ServoTicks,
}