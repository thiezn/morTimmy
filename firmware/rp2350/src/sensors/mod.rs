//! Peripheral state for sensor inputs.

pub mod battery;
pub mod ultrasonic;

use mortimmy_core::Millimeters;
use mortimmy_protocol::messages::{BatteryTelemetry, RangeTelemetry};

use self::battery::BatteryMonitorTask;
use self::ultrasonic::UltrasonicTask;

/// Aggregate sensor state for board bring-up and unit tests.
#[derive(Clone, Copy, Debug, Default)]
pub struct SensorSuite {
    /// Battery monitor state.
    pub battery: BatteryMonitorTask,
    /// Ultrasonic sensor state.
    pub ultrasonic: UltrasonicTask,
}

impl SensorSuite {
    /// Record a new ultrasonic ranging sample.
    pub fn record_range(&mut self, distance_mm: Millimeters, quality: u8) -> RangeTelemetry {
        self.ultrasonic.record_measurement(distance_mm, quality)
    }

    /// Record a new battery monitor sample.
    pub fn record_battery(&mut self, millivolts: u16) -> BatteryTelemetry {
        self.battery.record_sample(millivolts)
    }
}