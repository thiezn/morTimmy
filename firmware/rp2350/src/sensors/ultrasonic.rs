use mortimmy_core::Millimeters;
use mortimmy_protocol::messages::telemetry::{
    ForwardRangeTelemetry, RangeSensorPosition, RangeTelemetry,
};

/// Runtime state for ultrasonic ranging.
#[derive(Clone, Copy, Debug, Default)]
pub struct UltrasonicTask {
    /// Whether ultrasonic sensing has been exercised.
    pub enabled: bool,
    /// Latest ultrasonic ranging samples for the forward sensor pair.
    pub ranges: ForwardRangeTelemetry,
}

impl UltrasonicTask {
    /// Record a new ultrasonic ranging sample.
    pub fn record_measurement(
        &mut self,
        sensor: RangeSensorPosition,
        distance_mm: Millimeters,
        quality: u8,
    ) -> RangeTelemetry {
        self.enabled = true;
        let telemetry = RangeTelemetry {
            sensor,
            distance_mm,
            quality,
        };

        match sensor {
            RangeSensorPosition::ForwardLeft => self.ranges.forward_left = Some(telemetry),
            RangeSensorPosition::ForwardRight => self.ranges.forward_right = Some(telemetry),
        }

        telemetry
    }
}
