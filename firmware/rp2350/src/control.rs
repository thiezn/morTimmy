#![allow(dead_code)]

use mortimmy_core::{
    CoreError, DEFAULT_LIMITS, Milliseconds, Mode, PwmTicks, RobotLimits, ServoTicks,
};
use mortimmy_protocol::messages::commands::{
    DesiredStateCommand, DriveCommand, ParameterKey, ParameterUpdate, ServoCommand,
};

use crate::{
    actuators::{drive::DriveTrainState, servo::PanTiltServoState},
    board::{BoardProfile, active_board_profile},
};

/// Top-level control loop state for the embedded target.
#[derive(Clone, Copy, Debug)]
pub struct ControlLoop {
    /// Current robot mode.
    pub mode: Mode,
    /// Active safety limits.
    pub limits: RobotLimits,
    /// Current board profile.
    pub board: BoardProfile,
    /// Differential drive actuator state.
    pub drive: DriveTrainState,
    /// Pan/tilt servo actuator state.
    pub servo: PanTiltServoState,
    /// Last protocol-level error that affected the control plane.
    pub last_error: Option<CoreError>,
}

impl ControlLoop {
    /// Construct the control loop with safe defaults.
    pub const fn new() -> Self {
        Self {
            mode: Mode::Teleop,
            limits: DEFAULT_LIMITS,
            board: active_board_profile(),
            drive: DriveTrainState::stopped(),
            servo: PanTiltServoState::centered(),
            last_error: None,
        }
    }

    /// Apply a drive command after firmware-side safety clamping.
    pub fn apply_drive(&mut self, command: DriveCommand) {
        self.drive.apply_command(command, self.limits.max_drive_pwm);
        self.last_error = None;
    }

    /// Apply a servo command after clamping its delta against the current limit.
    pub fn apply_servo(&mut self, command: ServoCommand) {
        self.servo
            .apply_command(command, self.limits.max_servo_step);
        self.last_error = None;
    }

    /// Apply the latest desired control snapshot.
    pub fn apply_desired_state(&mut self, desired_state: DesiredStateCommand) {
        self.mode = desired_state.mode;

        if desired_state.mode == Mode::Fault {
            self.drive.stop();
            self.servo = PanTiltServoState::centered();
            self.last_error = None;
            return;
        }

        self.apply_servo(desired_state.servo());
        self.apply_drive(desired_state.drive());
    }

    /// Record a protocol-level control error.
    pub fn record_error(&mut self, error: CoreError) {
        self.last_error = Some(error);
    }

    /// Apply limit-related parameter updates that belong to the control loop.
    pub fn apply_limit_parameter(&mut self, update: ParameterUpdate) -> Result<bool, CoreError> {
        match update.key {
            ParameterKey::MaxDrivePwm => {
                let Some(value) = clamp_positive_i16(update.value) else {
                    return Err(CoreError::InvalidCommand);
                };
                self.limits.max_drive_pwm = PwmTicks(value);
                self.apply_drive(self.drive.command());
                Ok(true)
            }
            ParameterKey::MaxServoStep => {
                let Some(value) = clamp_positive_u16(update.value) else {
                    return Err(CoreError::InvalidCommand);
                };
                self.limits.max_servo_step = ServoTicks(value);
                self.apply_servo(self.servo.command());
                Ok(true)
            }
            ParameterKey::LinkTimeoutMs => {
                let Some(value) = clamp_positive_u32(update.value) else {
                    return Err(CoreError::InvalidCommand);
                };
                self.limits.link_timeout_ms = Milliseconds(value);
                self.last_error = None;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

impl Default for ControlLoop {
    fn default() -> Self {
        Self::new()
    }
}

fn clamp_positive_i16(value: i32) -> Option<i16> {
    (0..=i32::from(i16::MAX))
        .contains(&value)
        .then_some(value as i16)
}

fn clamp_positive_u16(value: i32) -> Option<u16> {
    (0..=i32::from(u16::MAX))
        .contains(&value)
        .then_some(value as u16)
}

fn clamp_positive_u32(value: i32) -> Option<u32> {
    (0..=i32::MAX)
        .contains(&value)
        .then_some(value as u32)
        .filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use mortimmy_core::{Mode, PwmTicks, ServoTicks};
    use mortimmy_protocol::messages::commands::{
        DesiredStateCommand, DriveCommand, ParameterKey, ParameterUpdate, ServoCommand,
    };

    use super::ControlLoop;

    #[test]
    fn applies_drive_command_inside_limits() {
        let mut control = ControlLoop::default();
        control.apply_drive(DriveCommand {
            left: PwmTicks(250),
            right: PwmTicks(-300),
        });

        assert_eq!(control.drive.left_pwm.0, 250);
        assert_eq!(control.drive.right_pwm.0, -300);
    }

    #[test]
    fn clamps_servo_steps_to_configured_limit() {
        let mut control = ControlLoop::default();
        control.apply_servo(ServoCommand {
            pan: ServoTicks(120),
            tilt: ServoTicks(100),
        });

        assert_eq!(control.servo.pan, ServoTicks(48));
        assert_eq!(control.servo.tilt, ServoTicks(48));
    }

    #[test]
    fn desired_state_in_fault_restores_safe_stopped_state() {
        let mut control = ControlLoop::default();
        control.apply_drive(DriveCommand {
            left: PwmTicks(400),
            right: PwmTicks(400),
        });

        control.apply_desired_state(DesiredStateCommand::new(
            Mode::Fault,
            DriveCommand {
                left: PwmTicks(250),
                right: PwmTicks(250),
            },
            ServoCommand {
                pan: ServoTicks(12),
                tilt: ServoTicks(18),
            },
        ));

        assert_eq!(control.mode, Mode::Fault);
        assert_eq!(control.drive.left_pwm.0, 0);
        assert_eq!(control.drive.right_pwm.0, 0);
        assert_eq!(control.servo.pan, ServoTicks(0));
        assert_eq!(control.servo.tilt, ServoTicks(0));
    }

    #[test]
    fn desired_state_keeps_stationary_teleop_distinct_from_fault() {
        let mut control = ControlLoop::default();

        control.apply_desired_state(DesiredStateCommand::new(
            Mode::Teleop,
            DriveCommand {
                left: PwmTicks(0),
                right: PwmTicks(0),
            },
            ServoCommand {
                pan: ServoTicks(24),
                tilt: ServoTicks(36),
            },
        ));

        assert_eq!(control.mode, Mode::Teleop);
        assert_eq!(control.drive.left_pwm.0, 0);
        assert_eq!(control.drive.right_pwm.0, 0);
        assert_eq!(control.servo.pan, ServoTicks(24));
        assert_eq!(control.servo.tilt, ServoTicks(36));
    }

    #[test]
    fn applies_limit_parameter_updates() {
        let mut control = ControlLoop::default();

        assert!(
            control
                .apply_limit_parameter(ParameterUpdate {
                    key: ParameterKey::LinkTimeoutMs,
                    value: 500,
                })
                .unwrap()
        );
        assert_eq!(control.limits.link_timeout_ms.0, 500);
    }

    #[test]
    fn rejects_invalid_limit_parameter_updates() {
        let mut control = ControlLoop::default();

        assert_eq!(
            control.apply_limit_parameter(ParameterUpdate {
                key: ParameterKey::MaxDrivePwm,
                value: -1,
            }),
            Err(mortimmy_core::CoreError::InvalidCommand)
        );
    }
}
