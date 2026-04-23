use mortimmy_core::Millimeters;
use mortimmy_protocol::messages::telemetry::RangeTelemetry;

/// Runtime state for ultrasonic ranging.
#[derive(Clone, Copy, Debug, Default)]
pub struct UltrasonicTask {
    /// Whether ultrasonic sensing has been exercised.
    pub enabled: bool,
    /// Last ultrasonic ranging sample.
    pub last_sample: Option<RangeTelemetry>,
}

impl UltrasonicTask {
    /// Record a new ultrasonic ranging sample.
    pub fn record_measurement(&mut self, distance_mm: Millimeters, quality: u8) -> RangeTelemetry {
        self.enabled = true;
        let telemetry = RangeTelemetry {
            distance_mm,
            quality,
        };
        self.last_sample = Some(telemetry);
        telemetry
    }
}
