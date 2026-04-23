use mortimmy_core::PwmTicks;
use mortimmy_drivers::{MotorDriver, MotorPowerCommand};
use mortimmy_protocol::messages::{
    commands::DriveCommand,
    telemetry::MotorStateTelemetry,
};

/// Errors returned while applying drive state to a concrete hardware driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriveHardwareError<DriverError> {
    InvalidDriveRange,
    Driver(DriverError),
}

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
        let max_drive_pwm = i32::from(max_drive_pwm.0)
            .unsigned_abs()
            .min(i16::MAX as u32) as i16;
        let left_pwm = command.left.0.clamp(-max_drive_pwm, max_drive_pwm);
        let right_pwm = command.right.0.clamp(-max_drive_pwm, max_drive_pwm);

        self.left_pwm = PwmTicks(left_pwm);
        self.right_pwm = PwmTicks(right_pwm);
        self.current_limit_hit = left_pwm != command.left.0 || right_pwm != command.right.0;
    }

    /// Apply the currently clamped state to a concrete differential-drive driver.
    pub fn apply_to_driver<D>(
        &self,
        driver: &mut D,
        max_drive_pwm: PwmTicks,
    ) -> Result<(), DriveHardwareError<D::Error>>
    where
        D: MotorDriver,
    {
        if self.left_pwm.0 == 0 && self.right_pwm.0 == 0 {
            return driver.stop_all().map_err(DriveHardwareError::Driver);
        }

        let max_drive_pwm = i32::from(max_drive_pwm.0)
            .unsigned_abs()
            .min(i16::MAX as u32) as u16;
        if max_drive_pwm == 0 {
            return Err(DriveHardwareError::InvalidDriveRange);
        }

        let left = MotorPowerCommand::new(self.left_pwm, max_drive_pwm)
            .ok_or(DriveHardwareError::InvalidDriveRange)?;
        let right = MotorPowerCommand::new(self.right_pwm, max_drive_pwm)
            .ok_or(DriveHardwareError::InvalidDriveRange)?;

        driver
            .set_outputs(left, right)
            .map_err(DriveHardwareError::Driver)
    }

    /// Clamp one drive command into state and immediately forward it to the hardware driver.
    pub fn apply_command_with_driver<D>(
        &mut self,
        command: DriveCommand,
        max_drive_pwm: PwmTicks,
        driver: &mut D,
    ) -> Result<(), DriveHardwareError<D::Error>>
    where
        D: MotorDriver,
    {
        self.apply_command(command, max_drive_pwm);
        self.apply_to_driver(driver, max_drive_pwm)
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

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use mortimmy_drivers::{MotorChannel, MotorDriver, MotorPowerCommand};

    use super::{DriveHardwareError, DriveTrainState};
    use mortimmy_core::PwmTicks;
    use mortimmy_protocol::messages::commands::DriveCommand;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum FakeDriverError {
        Injected,
    }

    #[derive(Debug, Default)]
    struct RecordingDriver {
        outputs: Vec<(MotorChannel, MotorPowerCommand)>,
        stop_calls: usize,
        fail: bool,
    }

    impl MotorDriver for RecordingDriver {
        type Error = FakeDriverError;

        fn set_output(
            &mut self,
            channel: MotorChannel,
            command: MotorPowerCommand,
        ) -> Result<(), Self::Error> {
            self.outputs.push((channel, command));
            if self.fail {
                return Err(FakeDriverError::Injected);
            }
            Ok(())
        }

        fn stop_all(&mut self) -> Result<(), Self::Error> {
            self.stop_calls += 1;
            if self.fail {
                return Err(FakeDriverError::Injected);
            }
            Ok(())
        }
    }

    #[test]
    fn apply_command_marks_current_limit_when_clamped() {
        let mut state = DriveTrainState::default();

        state.apply_command(
            DriveCommand {
                left: PwmTicks(1_200),
                right: PwmTicks(-250),
            },
            PwmTicks(1_000),
        );

        assert_eq!(state.left_pwm, PwmTicks(1_000));
        assert_eq!(state.right_pwm, PwmTicks(-250));
        assert!(state.current_limit_hit);
    }

    #[test]
    fn apply_to_driver_routes_state_through_motor_driver() {
        let mut state = DriveTrainState::default();
        let mut driver = RecordingDriver::default();

        state.apply_command(
            DriveCommand {
                left: PwmTicks(500),
                right: PwmTicks(-300),
            },
            PwmTicks(1_000),
        );
        state.apply_to_driver(&mut driver, PwmTicks(1_000)).unwrap();

        assert_eq!(driver.stop_calls, 0);
        assert_eq!(driver.outputs.len(), 2);
        assert_eq!(driver.outputs[0].0, MotorChannel::Left);
        assert_eq!(
            driver.outputs[0].1,
            MotorPowerCommand::new(PwmTicks(500), 1_000).unwrap()
        );
        assert_eq!(driver.outputs[1].0, MotorChannel::Right);
        assert_eq!(
            driver.outputs[1].1,
            MotorPowerCommand::new(PwmTicks(-300), 1_000).unwrap()
        );
    }

    #[test]
    fn apply_to_driver_stops_when_state_is_stationary() {
        let state = DriveTrainState::stopped();
        let mut driver = RecordingDriver::default();

        state.apply_to_driver(&mut driver, PwmTicks(1_000)).unwrap();

        assert_eq!(driver.stop_calls, 1);
        assert!(driver.outputs.is_empty());
    }

    #[test]
    fn apply_command_with_driver_propagates_driver_failures() {
        let mut state = DriveTrainState::default();
        let mut driver = RecordingDriver {
            fail: true,
            ..RecordingDriver::default()
        };

        let result = state.apply_command_with_driver(
            DriveCommand {
                left: PwmTicks(100),
                right: PwmTicks(100),
            },
            PwmTicks(1_000),
            &mut driver,
        );

        assert_eq!(
            result,
            Err(DriveHardwareError::Driver(FakeDriverError::Injected))
        );
    }
}
