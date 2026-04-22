use mortimmy_core::PwmTicks;
use mortimmy_protocol::messages::{DriveCommand, MotorStateTelemetry};

/// Runtime state for the differential drive outputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DriveTrainState {
    /// Current left motor PWM request.
    pub left_pwm: PwmTicks,
    /// Current right motor PWM request.
    pub right_pwm: PwmTicks,
    /// Whether current limiting clipped the last request.
    pub current_limit_hit: bool,
}

impl DriveTrainState {
    /// Construct a stopped drive train.
    pub const fn stopped() -> Self {
        Self {
            left_pwm: PwmTicks(0),
            right_pwm: PwmTicks(0),
            current_limit_hit: false,
        }
    }

    /// Apply a drive command after clamping it against the configured limit.
    pub fn apply_command(&mut self, command: DriveCommand, max_drive_pwm: PwmTicks) {
        let max_drive_pwm = max_drive_pwm.0;
        self.left_pwm = PwmTicks(command.left.0.clamp(-max_drive_pwm, max_drive_pwm));
        self.right_pwm = PwmTicks(command.right.0.clamp(-max_drive_pwm, max_drive_pwm));
        self.current_limit_hit = false;
    }

    /// Stop both drive outputs.
    pub fn stop(&mut self) {
        *self = Self::stopped();
    }

    /// Render the current drive request as a protocol command.
    pub const fn command(&self) -> DriveCommand {
        DriveCommand {
            left: self.left_pwm,
            right: self.right_pwm,
        }
    }

    /// Render the current drive state as protocol telemetry.
    pub const fn telemetry(&self) -> MotorStateTelemetry {
        MotorStateTelemetry {
            left_pwm: self.left_pwm,
            right_pwm: self.right_pwm,
            current_limit_hit: self.current_limit_hit,
        }
    }
}

impl Default for DriveTrainState {
    fn default() -> Self {
        Self::stopped()
    }
}