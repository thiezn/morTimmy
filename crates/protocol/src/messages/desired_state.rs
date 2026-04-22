use mortimmy_core::{CoreError, Mode};
use serde::{Deserialize, Serialize};

use super::{DriveCommand, MotorStateTelemetry, ServoCommand, ServoStateTelemetry};

/// Full continuous-control snapshot owned by the host brain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesiredStateCommand {
    pub mode: Mode,
    pub drive: DriveCommand,
    pub servo: ServoCommand,
}

impl DesiredStateCommand {
    pub const fn new(mode: Mode, drive: DriveCommand, servo: ServoCommand) -> Self {
        Self { mode, drive, servo }
    }
}

/// Immediate acknowledgement of the latest applied desired-control state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesiredStateTelemetry {
    pub mode: Mode,
    pub drive: MotorStateTelemetry,
    pub servo: ServoStateTelemetry,
    pub error: Option<CoreError>,
}
