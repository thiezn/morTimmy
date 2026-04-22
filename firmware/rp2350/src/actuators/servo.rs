use mortimmy_core::ServoTicks;
use mortimmy_protocol::messages::{ServoCommand, ServoStateTelemetry};

/// Runtime state for the pan/tilt servo pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanTiltServoState {
    /// Current pan position.
    pub pan: ServoTicks,
    /// Current tilt position.
    pub tilt: ServoTicks,
}

impl PanTiltServoState {
    /// Construct a centered pan/tilt pair.
    pub const fn centered() -> Self {
        Self {
            pan: ServoTicks(0),
            tilt: ServoTicks(0),
        }
    }

    /// Apply a servo command after clamping its per-axis delta against the configured limit.
    pub fn apply_command(&mut self, command: ServoCommand, max_step: ServoTicks) {
        self.pan = clamp_servo_axis(self.pan, command.pan, max_step);
        self.tilt = clamp_servo_axis(self.tilt, command.tilt, max_step);
    }

    /// Render the current servo request as a protocol command.
    pub const fn command(&self) -> ServoCommand {
        ServoCommand {
            pan: self.pan,
            tilt: self.tilt,
        }
    }

    /// Render the current servo state as protocol telemetry.
    pub const fn telemetry(&self) -> ServoStateTelemetry {
        ServoStateTelemetry {
            pan: self.pan,
            tilt: self.tilt,
        }
    }
}

impl Default for PanTiltServoState {
    fn default() -> Self {
        Self::centered()
    }
}

fn clamp_servo_axis(current: ServoTicks, requested: ServoTicks, max_step: ServoTicks) -> ServoTicks {
    let current = i32::from(current.0);
    let requested = i32::from(requested.0);
    let max_step = i32::from(max_step.0);
    let lower = current.saturating_sub(max_step);
    let upper = current.saturating_add(max_step);
    ServoTicks(requested.clamp(lower, upper) as u16)
}