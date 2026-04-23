use mortimmy_core::PwmTicks;

pub mod l298n;

/// Identifies a logical differential-drive side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotorChannel {
    Left,
    Right,
}

/// Signed direction derived from a motor power command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotorDirection {
    Forward,
    Reverse,
    Stop,
}

/// Preferred electrical behavior when commanding a motor channel to stop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MotorStopMode {
    /// Disable the bridge and let the motor coast down naturally.
    #[default]
    Coast,
    /// Actively short the motor through the bridge to brake it.
    Brake,
}

/// A signed motor request together with the logical full-scale input range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotorPowerCommand {
    output: PwmTicks,
    max_output: u16,
}

impl MotorPowerCommand {
    /// Construct a command only when the output fits inside the declared full-scale range.
    pub fn new(output: PwmTicks, max_output: u16) -> Option<Self> {
        if max_output == 0 {
            return None;
        }

        let max_output = max_output.min(i16::MAX as u16);
        let output = i32::from(output.0);
        let max_output_i32 = i32::from(max_output);

        ((-max_output_i32)..=max_output_i32)
            .contains(&output)
            .then_some(Self {
                output: PwmTicks(output as i16),
                max_output,
            })
    }

    /// Return the signed logical output.
    pub const fn output(self) -> PwmTicks {
        self.output
    }

    /// Return the declared logical full-scale range.
    pub const fn max_output(self) -> u16 {
        self.max_output
    }

    /// Whether the command requests a stationary output.
    pub const fn is_stop(self) -> bool {
        self.output.0 == 0
    }

    /// Return the requested direction.
    pub const fn direction(self) -> MotorDirection {
        if self.output.0 > 0 {
            MotorDirection::Forward
        } else if self.output.0 < 0 {
            MotorDirection::Reverse
        } else {
            MotorDirection::Stop
        }
    }

    /// Convert the signed request into a duty cycle for a concrete PWM peripheral.
    pub fn duty_for(self, max_duty: u16) -> u16 {
        if self.is_stop() || max_duty == 0 {
            return 0;
        }

        let magnitude = i32::from(self.output.0).unsigned_abs();
        let duty = magnitude * u32::from(max_duty) / u32::from(self.max_output);
        duty.min(u32::from(max_duty)) as u16
    }
}

/// Trait implemented by logical differential-drive motor drivers.
pub trait MotorDriver {
    /// Driver-specific error type.
    type Error;

    /// Set one logical drive side to the requested signed output.
    fn set_output(
        &mut self,
        channel: MotorChannel,
        command: MotorPowerCommand,
    ) -> Result<(), Self::Error>;

    /// Apply both logical drive sides, stopping everything if the second write fails.
    fn set_outputs(
        &mut self,
        left: MotorPowerCommand,
        right: MotorPowerCommand,
    ) -> Result<(), Self::Error> {
        self.set_output(MotorChannel::Left, left)?;

        if let Err(error) = self.set_output(MotorChannel::Right, right) {
            let _ = self.stop_all();
            return Err(error);
        }

        Ok(())
    }

    /// Stop every managed motor channel.
    fn stop_all(&mut self) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::{MotorChannel, MotorDriver, MotorPowerCommand};
    use mortimmy_core::PwmTicks;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum FakeDriverError {
        Injected,
    }

    #[derive(Debug, Default)]
    struct RecordingMotorDriver {
        outputs: Vec<(MotorChannel, MotorPowerCommand)>,
        stop_calls: usize,
        fail_right: bool,
    }

    impl MotorDriver for RecordingMotorDriver {
        type Error = FakeDriverError;

        fn set_output(
            &mut self,
            channel: MotorChannel,
            command: MotorPowerCommand,
        ) -> Result<(), Self::Error> {
            self.outputs.push((channel, command));

            if self.fail_right && channel == MotorChannel::Right {
                return Err(FakeDriverError::Injected);
            }

            Ok(())
        }

        fn stop_all(&mut self) -> Result<(), Self::Error> {
            self.stop_calls += 1;
            Ok(())
        }
    }

    #[test]
    fn command_scales_to_pwm_duty() {
        let command = MotorPowerCommand::new(PwmTicks(500), 1_000).unwrap();

        assert_eq!(command.duty_for(255), 127);
    }

    #[test]
    fn command_rejects_out_of_range_requests() {
        assert!(MotorPowerCommand::new(PwmTicks(1_200), 1_000).is_none());
        assert!(MotorPowerCommand::new(PwmTicks(0), 0).is_none());
    }

    #[test]
    fn set_outputs_stops_everything_when_second_channel_fails() {
        let mut driver = RecordingMotorDriver {
            fail_right: true,
            ..RecordingMotorDriver::default()
        };

        let left = MotorPowerCommand::new(PwmTicks(300), 1_000).unwrap();
        let right = MotorPowerCommand::new(PwmTicks(-300), 1_000).unwrap();

        assert_eq!(driver.set_outputs(left, right), Err(FakeDriverError::Injected));
        assert_eq!(driver.stop_calls, 1);
        assert_eq!(driver.outputs.len(), 2);
    }
}
