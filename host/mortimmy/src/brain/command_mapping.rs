//! Host-side mapping from control state into protocol commands.

use mortimmy_core::{DEFAULT_LIMITS, Mode, PwmTicks, RobotLimits, ServoTicks};
use mortimmy_protocol::messages::{
    command::Command,
    commands::{DesiredStateCommand, DriveCommand, ParameterKey, ParameterUpdate, ServoCommand},
};

use crate::input::DriveIntent;

/// Host-owned defaults and safety limits used to build protocol commands.
#[derive(Clone, Copy, Debug)]
pub struct RouterPolicy {
    pub default_mode: Mode,
    pub limits: RobotLimits,
}

impl Default for RouterPolicy {
    fn default() -> Self {
        Self {
            default_mode: Mode::Teleop,
            limits: DEFAULT_LIMITS,
        }
    }
}

impl RouterPolicy {
    /// Clamp a drive request into the configured safety window.
    pub fn clamp_drive(&self, left: i16, right: i16) -> DriveCommand {
        let max_drive_pwm = self.limits.max_drive_pwm.0;

        DriveCommand {
            left: PwmTicks(left.clamp(-max_drive_pwm, max_drive_pwm)),
            right: PwmTicks(right.clamp(-max_drive_pwm, max_drive_pwm)),
        }
    }

    /// Build the default centered servo target.
    pub const fn centered_servo() -> ServoCommand {
        ServoCommand {
            pan: ServoTicks(0),
            tilt: ServoTicks(0),
        }
    }

    /// Build a full desired control snapshot from the current host-owned state.
    pub fn desired_state_command(
        &self,
        mode: Mode,
        drive: Option<DriveIntent>,
        servo: ServoCommand,
    ) -> Command {
        let drive = match drive {
            Some(intent) => self.drive_intent(intent),
            None => self.clamp_drive(0, 0),
        };

        Command::SetDesiredState(DesiredStateCommand::new(mode, drive, servo))
    }

    /// Convert a normalized drive intent into differential motor PWM values.
    pub fn drive_intent(&self, intent: DriveIntent) -> DriveCommand {
        let forward = i32::from(
            intent
                .forward
                .clamp(-DriveIntent::AXIS_MAX, DriveIntent::AXIS_MAX),
        );
        let turn = i32::from(
            intent
                .turn
                .clamp(-DriveIntent::AXIS_MAX, DriveIntent::AXIS_MAX),
        );
        let speed = i32::from(intent.speed.min(self.limits.max_drive_pwm.0 as u16));

        let left = forward + turn;
        let right = forward - turn;
        let normalizer = left
            .abs()
            .max(right.abs())
            .max(i32::from(DriveIntent::AXIS_MAX));

        let scaled_left = left * speed / normalizer;
        let scaled_right = right * speed / normalizer;

        self.clamp_drive(scaled_left as i16, scaled_right as i16)
    }

    /// Build a command that updates the firmware link timeout.
    pub const fn link_timeout_update(milliseconds: u32) -> Command {
        Command::SetParam(ParameterUpdate {
            key: ParameterKey::LinkTimeoutMs,
            value: milliseconds as i32,
        })
    }
}

#[cfg(test)]
mod tests {
    use mortimmy_core::{Mode, ServoTicks};
    use mortimmy_protocol::messages::command::Command;
    use mortimmy_protocol::messages::commands::{
        DesiredStateCommand, DriveCommand, ParameterKey, ParameterUpdate, ServoCommand,
    };

    use crate::input::DriveIntent;

    use super::RouterPolicy;

    #[test]
    fn clamps_drive_to_limits() {
        let router = RouterPolicy::default();
        let command = router.clamp_drive(2_000, -2_000);

        assert_eq!(command.left.0, 1_000);
        assert_eq!(command.right.0, -1_000);
    }

    #[test]
    fn preserves_in_range_drive() {
        let router = RouterPolicy::default();
        let command = router.clamp_drive(250, -320);

        assert_eq!(command.left.0, 250);
        assert_eq!(command.right.0, -320);
    }

    #[test]
    fn builds_control_messages() {
        assert_eq!(
            RouterPolicy::link_timeout_update(750),
            Command::SetParam(ParameterUpdate {
                key: ParameterKey::LinkTimeoutMs,
                value: 750,
            })
        );
    }

    #[test]
    fn converts_drive_intent_into_differential_pwm() {
        let router = RouterPolicy::default();

        assert_eq!(
            router.drive_intent(DriveIntent {
                forward: DriveIntent::AXIS_MAX,
                turn: 0,
                speed: 300,
            }),
            DriveCommand {
                left: mortimmy_core::PwmTicks(300),
                right: mortimmy_core::PwmTicks(300),
            }
        );

        assert_eq!(
            router.desired_state_command(
                Mode::Teleop,
                Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
                RouterPolicy::centered_servo(),
            ),
            Command::SetDesiredState(DesiredStateCommand::new(
                Mode::Teleop,
                DriveCommand {
                    left: mortimmy_core::PwmTicks(300),
                    right: mortimmy_core::PwmTicks(300),
                },
                ServoCommand {
                    pan: ServoTicks(0),
                    tilt: ServoTicks(0),
                },
            ))
        );

        assert_eq!(
            router.drive_intent(DriveIntent {
                forward: DriveIntent::AXIS_MAX,
                turn: -DriveIntent::AXIS_MAX,
                speed: 300,
            }),
            DriveCommand {
                left: mortimmy_core::PwmTicks(0),
                right: mortimmy_core::PwmTicks(300),
            }
        );
    }
}
