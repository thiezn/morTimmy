use mortimmy_protocol::messages::BatteryTelemetry;

/// Runtime state for battery monitoring.
#[derive(Clone, Copy, Debug, Default)]
pub struct BatteryMonitorTask {
    /// Whether battery monitoring has been exercised.
    pub enabled: bool,
    /// Last battery telemetry sample.
    pub last_sample: Option<BatteryTelemetry>,
}

impl BatteryMonitorTask {
    /// Record a new battery monitor sample.
    pub fn record_sample(&mut self, millivolts: u16) -> BatteryTelemetry {
        self.enabled = true;
        let telemetry = BatteryTelemetry { millivolts };
        self.last_sample = Some(telemetry);
        telemetry
    }
}