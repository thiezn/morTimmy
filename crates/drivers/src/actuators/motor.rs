use mortimmy_core::PwmTicks;

/// Identifies a drive motor channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotorChannel {
    Left,
    Right,
}

/// Trait implemented by motor drivers.
pub trait MotorDriver {
    /// Driver-specific error type.
    type Error;

    /// Set the signed PWM request for one motor channel.
    fn set_speed(&mut self, channel: MotorChannel, speed: PwmTicks) -> Result<(), Self::Error>;

    /// Stop every managed motor channel.
    fn stop_all(&mut self) -> Result<(), Self::Error>;
}