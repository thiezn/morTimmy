use mortimmy_core::PwmTicks;
use serde::{Deserialize, Serialize};

/// Current motor state after host clamping and firmware safety checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MotorStateTelemetry {
    pub left_pwm: PwmTicks,
    pub right_pwm: PwmTicks,
    pub current_limit_hit: bool,
}