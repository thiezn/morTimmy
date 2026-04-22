use mortimmy_core::PwmTicks;
use serde::{Deserialize, Serialize};

/// Differential motor PWM request for the left and right drive channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveCommand {
    pub left: PwmTicks,
    pub right: PwmTicks,
}

/// Current motor state after host clamping and firmware safety checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MotorStateTelemetry {
    pub left_pwm: PwmTicks,
    pub right_pwm: PwmTicks,
    pub current_limit_hit: bool,
}