use serde::{Deserialize, Serialize};

use crate::{Milliseconds, PwmTicks, ServoTicks};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobotLimits {
    pub max_drive_pwm: PwmTicks,
    pub max_servo_step: ServoTicks,
    pub link_timeout_ms: Milliseconds,
}

impl Default for RobotLimits {
    fn default() -> Self {
        DEFAULT_LIMITS
    }
}

pub const DEFAULT_LIMITS: RobotLimits = RobotLimits {
    max_drive_pwm: PwmTicks(1000),
    max_servo_step: ServoTicks(48),
    link_timeout_ms: Milliseconds(250),
};
