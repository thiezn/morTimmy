use embedded_hal::{
    digital::OutputPin,
    pwm::SetDutyCycle,
};

use super::{MotorChannel, MotorDirection, MotorDriver, MotorPowerCommand, MotorStopMode};

/// Invert a motor channel when the wiring causes the logical forward direction to spin backwards.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MotorPolarity {
    #[default]
    Normal,
    Inverted,
}

/// Per-channel L298N behavior configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct L298nChannelConfig {
    pub polarity: MotorPolarity,
    pub stop_mode: MotorStopMode,
}

/// Errors returned by the L298N driver implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum L298nError<PinError, PwmError> {
    Direction(PinError),
    Enable(PwmError),
}

/// One L298N bridge channel consisting of two direction pins and one PWM enable pin.
#[derive(Debug)]
pub struct L298nBridge<In1, In2, Enable> {
    in1: In1,
    in2: In2,
    enable: Enable,
    config: L298nChannelConfig,
}

impl<In1, In2, Enable> L298nBridge<In1, In2, Enable> {
    /// Construct a bridge with default polarity and coast-on-stop behavior.
    pub fn new(in1: In1, in2: In2, enable: Enable) -> Self {
        Self::with_config(in1, in2, enable, L298nChannelConfig::default())
    }

    /// Construct a bridge with explicit polarity and stop-mode behavior.
    pub const fn with_config(
        in1: In1,
        in2: In2,
        enable: Enable,
        config: L298nChannelConfig,
    ) -> Self {
        Self {
            in1,
            in2,
            enable,
            config,
        }
    }
}

impl<In1, In2, Enable, PinError, PwmError> L298nBridge<In1, In2, Enable>
where
    In1: OutputPin<Error = PinError>,
    In2: OutputPin<Error = PinError>,
    Enable: SetDutyCycle<Error = PwmError>,
{
    /// Drive this bridge according to one signed motor command.
    pub fn drive(
        &mut self,
        command: MotorPowerCommand,
    ) -> Result<(), L298nError<PinError, PwmError>> {
        if command.is_stop() {
            return self.stop();
        }

        self.enable
            .set_duty_cycle_fully_off()
            .map_err(L298nError::Enable)?;

        match self.effective_direction(command.direction()) {
            MotorDirection::Forward => {
                self.in1.set_low().map_err(L298nError::Direction)?;
                self.in2.set_high().map_err(L298nError::Direction)?;
            }
            MotorDirection::Reverse => {
                self.in1.set_high().map_err(L298nError::Direction)?;
                self.in2.set_low().map_err(L298nError::Direction)?;
            }
            MotorDirection::Stop => return self.stop(),
        }

        self.enable
            .set_duty_cycle(command.duty_for(self.enable.max_duty_cycle()))
            .map_err(L298nError::Enable)
    }

    /// Stop this bridge using the configured coast or brake mode.
    pub fn stop(&mut self) -> Result<(), L298nError<PinError, PwmError>> {
        self.enable
            .set_duty_cycle_fully_off()
            .map_err(L298nError::Enable)?;

        match self.config.stop_mode {
            MotorStopMode::Coast => {
                self.in1.set_low().map_err(L298nError::Direction)?;
                self.in2.set_low().map_err(L298nError::Direction)?;
                Ok(())
            }
            MotorStopMode::Brake => {
                self.in1.set_high().map_err(L298nError::Direction)?;
                self.in2.set_high().map_err(L298nError::Direction)?;
                self.enable
                    .set_duty_cycle_fully_on()
                    .map_err(L298nError::Enable)
            }
        }
    }

    fn effective_direction(&self, direction: MotorDirection) -> MotorDirection {
        match (self.config.polarity, direction) {
            (_, MotorDirection::Stop) => MotorDirection::Stop,
            (MotorPolarity::Normal, direction) => direction,
            (MotorPolarity::Inverted, MotorDirection::Forward) => MotorDirection::Reverse,
            (MotorPolarity::Inverted, MotorDirection::Reverse) => MotorDirection::Forward,
        }
    }
}

/// One logical robot side backed by both L298N bridge channels on a single board.
#[derive(Debug)]
pub struct L298nSideDriver<MotorA, MotorB> {
    motor_a: MotorA,
    motor_b: MotorB,
}

impl<MotorA, MotorB> L298nSideDriver<MotorA, MotorB> {
    pub const fn new(motor_a: MotorA, motor_b: MotorB) -> Self {
        Self { motor_a, motor_b }
    }
}
impl<In1A, In2A, EnableA, In1B, In2B, EnableB, PinError, PwmError>
    L298nSideDriver<
        L298nBridge<In1A, In2A, EnableA>,
        L298nBridge<In1B, In2B, EnableB>,
    >
where
    In1A: OutputPin<Error = PinError>,
    In2A: OutputPin<Error = PinError>,
    EnableA: SetDutyCycle<Error = PwmError>,
    In1B: OutputPin<Error = PinError>,
    In2B: OutputPin<Error = PinError>,
    EnableB: SetDutyCycle<Error = PwmError>,
{
    pub fn set_speed(
        &mut self,
        command: MotorPowerCommand,
    ) -> Result<(), L298nError<PinError, PwmError>> {
        self.motor_a.drive(command)?;
        if let Err(error) = self.motor_b.drive(command) {
            let _ = self.stop();
            return Err(error);
        }

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), L298nError<PinError, PwmError>> {
        let first = self.motor_a.stop();
        let second = self.motor_b.stop();

        match (first, second) {
            (Err(error), _) => Err(error),
            (_, Err(error)) => Err(error),
            (Ok(()), Ok(())) => Ok(()),
        }
    }
}

/// Differential-drive implementation built from one mirrored L298N side per robot side.
#[derive(Debug)]
pub struct L298nDriveMotorDriver<LeftA, LeftB, RightA, RightB> {
    left: L298nSideDriver<LeftA, LeftB>,
    right: L298nSideDriver<RightA, RightB>,
}

impl<LeftA, LeftB, RightA, RightB> L298nDriveMotorDriver<LeftA, LeftB, RightA, RightB> {
    pub const fn new(
        left: L298nSideDriver<LeftA, LeftB>,
        right: L298nSideDriver<RightA, RightB>,
    ) -> Self {
        Self { left, right }
    }
}

impl<
        LeftIn1A,
        LeftIn2A,
        LeftEnableA,
        LeftIn1B,
        LeftIn2B,
        LeftEnableB,
        RightIn1A,
        RightIn2A,
        RightEnableA,
        RightIn1B,
        RightIn2B,
        RightEnableB,
        PinError,
        PwmError,
    > MotorDriver
    for L298nDriveMotorDriver<
        L298nBridge<LeftIn1A, LeftIn2A, LeftEnableA>,
        L298nBridge<LeftIn1B, LeftIn2B, LeftEnableB>,
        L298nBridge<RightIn1A, RightIn2A, RightEnableA>,
        L298nBridge<RightIn1B, RightIn2B, RightEnableB>,
    >
where
    LeftIn1A: OutputPin<Error = PinError>,
    LeftIn2A: OutputPin<Error = PinError>,
    LeftEnableA: SetDutyCycle<Error = PwmError>,
    LeftIn1B: OutputPin<Error = PinError>,
    LeftIn2B: OutputPin<Error = PinError>,
    LeftEnableB: SetDutyCycle<Error = PwmError>,
    RightIn1A: OutputPin<Error = PinError>,
    RightIn2A: OutputPin<Error = PinError>,
    RightEnableA: SetDutyCycle<Error = PwmError>,
    RightIn1B: OutputPin<Error = PinError>,
    RightIn2B: OutputPin<Error = PinError>,
    RightEnableB: SetDutyCycle<Error = PwmError>,
{
    type Error = L298nError<PinError, PwmError>;

    fn set_output(
        &mut self,
        channel: MotorChannel,
        command: MotorPowerCommand,
    ) -> Result<(), Self::Error> {
        match channel {
            MotorChannel::Left => self.left.set_speed(command),
            MotorChannel::Right => self.right.set_speed(command),
        }
    }

    fn stop_all(&mut self) -> Result<(), Self::Error> {
        self.left.stop()?;
        self.right.stop()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use embedded_hal::{
        digital::{ErrorType as DigitalErrorType, OutputPin},
        pwm::{ErrorType as PwmErrorType, SetDutyCycle},
    };

    use super::{
        L298nBridge, L298nChannelConfig, L298nDriveMotorDriver, L298nError, L298nSideDriver,
        MotorDriver, MotorPolarity,
    };
    use crate::{MotorChannel, MotorPowerCommand, MotorStopMode};
    use mortimmy_core::PwmTicks;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum FakeDigitalError {
        Injected,
    }

    impl embedded_hal::digital::Error for FakeDigitalError {
        fn kind(&self) -> embedded_hal::digital::ErrorKind {
            embedded_hal::digital::ErrorKind::Other
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum FakePwmError {
        Injected,
    }

    impl embedded_hal::pwm::Error for FakePwmError {
        fn kind(&self) -> embedded_hal::pwm::ErrorKind {
            embedded_hal::pwm::ErrorKind::Other
        }
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    struct FakePin {
        is_high: bool,
        fail: bool,
    }

    impl DigitalErrorType for FakePin {
        type Error = FakeDigitalError;
    }

    impl OutputPin for FakePin {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            if self.fail {
                return Err(FakeDigitalError::Injected);
            }
            self.is_high = false;
            Ok(())
        }

        fn set_high(&mut self) -> Result<(), Self::Error> {
            if self.fail {
                return Err(FakeDigitalError::Injected);
            }
            self.is_high = true;
            Ok(())
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FakePwm {
        max_duty: u16,
        duty: u16,
        fail: bool,
    }

    impl Default for FakePwm {
        fn default() -> Self {
            Self {
                max_duty: 255,
                duty: 0,
                fail: false,
            }
        }
    }

    impl PwmErrorType for FakePwm {
        type Error = FakePwmError;
    }

    impl SetDutyCycle for FakePwm {
        fn max_duty_cycle(&self) -> u16 {
            self.max_duty
        }

        fn set_duty_cycle(&mut self, duty: u16) -> Result<(), Self::Error> {
            if self.fail {
                return Err(FakePwmError::Injected);
            }
            self.duty = duty;
            Ok(())
        }
    }

    fn bridge_with_config(config: L298nChannelConfig) -> L298nBridge<FakePin, FakePin, FakePwm> {
        L298nBridge::with_config(FakePin::default(), FakePin::default(), FakePwm::default(), config)
    }

    #[test]
    fn bridge_sets_forward_direction_and_scaled_pwm() {
        let mut bridge = bridge_with_config(L298nChannelConfig::default());

        bridge
            .drive(MotorPowerCommand::new(PwmTicks(500), 1_000).unwrap())
            .unwrap();

        assert!(!bridge.in1.is_high);
        assert!(bridge.in2.is_high);
        assert_eq!(bridge.enable.duty, 127);
    }

    #[test]
    fn bridge_sets_reverse_direction_and_scaled_pwm() {
        let mut bridge = bridge_with_config(L298nChannelConfig::default());

        bridge
            .drive(MotorPowerCommand::new(PwmTicks(-250), 1_000).unwrap())
            .unwrap();

        assert!(bridge.in1.is_high);
        assert!(!bridge.in2.is_high);
        assert_eq!(bridge.enable.duty, 63);
    }

    #[test]
    fn bridge_supports_inverted_polarity() {
        let mut bridge = bridge_with_config(L298nChannelConfig {
            polarity: MotorPolarity::Inverted,
            stop_mode: MotorStopMode::Coast,
        });

        bridge
            .drive(MotorPowerCommand::new(PwmTicks(400), 1_000).unwrap())
            .unwrap();

        assert!(bridge.in1.is_high);
        assert!(!bridge.in2.is_high);
    }

    #[test]
    fn bridge_brakes_when_requested() {
        let mut bridge = bridge_with_config(L298nChannelConfig {
            polarity: MotorPolarity::Normal,
            stop_mode: MotorStopMode::Brake,
        });

        bridge.stop().unwrap();

        assert!(bridge.in1.is_high);
        assert!(bridge.in2.is_high);
        assert_eq!(bridge.enable.duty, bridge.enable.max_duty);
    }

    #[test]
    fn side_driver_mirrors_both_bridge_channels() {
        let left = bridge_with_config(L298nChannelConfig::default());
        let right = bridge_with_config(L298nChannelConfig::default());
        let mut side = L298nSideDriver::new(left, right);

        side.set_speed(MotorPowerCommand::new(PwmTicks(600), 1_000).unwrap())
            .unwrap();

        assert_eq!(side.motor_a.enable.duty, 153);
        assert_eq!(side.motor_b.enable.duty, 153);
        assert!(!side.motor_a.in1.is_high);
        assert!(side.motor_a.in2.is_high);
        assert!(!side.motor_b.in1.is_high);
        assert!(side.motor_b.in2.is_high);
    }

    #[test]
    fn drive_driver_routes_left_and_right_outputs() {
        let left_side = L298nSideDriver::new(
            bridge_with_config(L298nChannelConfig::default()),
            bridge_with_config(L298nChannelConfig::default()),
        );
        let right_side = L298nSideDriver::new(
            bridge_with_config(L298nChannelConfig::default()),
            bridge_with_config(L298nChannelConfig::default()),
        );
        let mut driver = L298nDriveMotorDriver::new(left_side, right_side);

        driver
            .set_output(
                MotorChannel::Left,
                MotorPowerCommand::new(PwmTicks(700), 1_000).unwrap(),
            )
            .unwrap();
        driver
            .set_output(
                MotorChannel::Right,
                MotorPowerCommand::new(PwmTicks(-200), 1_000).unwrap(),
            )
            .unwrap();

        assert_eq!(driver.left.motor_a.enable.duty, 178);
        assert_eq!(driver.right.motor_a.enable.duty, 51);
        assert!(!driver.left.motor_a.in1.is_high);
        assert!(driver.left.motor_a.in2.is_high);
        assert!(driver.right.motor_a.in1.is_high);
        assert!(!driver.right.motor_a.in2.is_high);
    }

    #[test]
    fn bridge_surfaces_pwm_failures() {
        let mut bridge = L298nBridge::new(
            FakePin::default(),
            FakePin::default(),
            FakePwm {
                fail: true,
                ..FakePwm::default()
            },
        );

        assert_eq!(
            bridge.drive(MotorPowerCommand::new(PwmTicks(100), 1_000).unwrap()),
            Err(L298nError::Enable(FakePwmError::Injected))
        );
    }
}