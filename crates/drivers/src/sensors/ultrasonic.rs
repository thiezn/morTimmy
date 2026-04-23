use mortimmy_core::Millimeters;

#[path = "ultrasonic/hc_sr04.rs"]
pub mod hc_sr04;

/// Trait implemented by ultrasonic ranging sensors.
pub trait UltrasonicSensor {
    /// Driver-specific error type.
    type Error;

    /// Trigger a measurement and return the range in millimeters.
    fn measure_range_mm(&mut self) -> Result<Millimeters, Self::Error>;
}
