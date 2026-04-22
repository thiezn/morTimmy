use mortimmy_core::ServoTicks;

/// Identifies one axis in a pan/tilt servo pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanTiltAxis {
    Pan,
    Tilt,
}

/// Trait implemented by servo drivers.
pub trait ServoDriver {
    /// Driver-specific error type.
    type Error;

    /// Set the target position for one servo axis.
    fn set_angle(&mut self, axis: PanTiltAxis, position: ServoTicks) -> Result<(), Self::Error>;

    /// Recenter the managed servo pair.
    fn center(&mut self) -> Result<(), Self::Error>;
}