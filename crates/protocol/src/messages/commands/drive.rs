use mortimmy_core::PwmTicks;
use serde::{Deserialize, Serialize};

/// Differential motor PWM request for the left and right drive channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveCommand {
    pub left: PwmTicks,
    pub right: PwmTicks,
}