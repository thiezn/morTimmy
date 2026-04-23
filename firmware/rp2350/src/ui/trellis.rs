#![allow(dead_code)]

use mortimmy_drivers::{PadEvent, PadEventKind as DriverPadEventKind, PadIndex};
use mortimmy_protocol::messages::telemetry::{PadEventKind, TrellisPadTelemetry};

/// Configuration for the Trellis M4 4x4 keypad and LED module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrellisConfig {
    /// Whether the Trellis module is enabled.
    pub enabled: bool,
    /// Poll interval in milliseconds.
    pub poll_interval_ms: u16,
    /// Global LED brightness.
    pub brightness: u8,
}

impl Default for TrellisConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            poll_interval_ms: 25,
            brightness: 32,
        }
    }
}

/// Placeholder Trellis task state for Qw/ST keypad bring-up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TrellisTask {
    /// Trellis runtime config.
    pub config: TrellisConfig,
    /// Current LED mask mirrored to the device.
    pub led_mask: u16,
    /// Last observed pad event.
    pub last_pad: Option<PadIndex>,
}

impl TrellisTask {
    /// Apply a new LED mask to the keypad matrix.
    pub fn apply_led_mask(&mut self, led_mask: u16) {
        self.led_mask = led_mask;
    }

    /// Record a Trellis pad event and convert it to protocol telemetry.
    pub fn record_pad_event(&mut self, event: PadEvent) -> TrellisPadTelemetry {
        self.config.enabled = true;
        self.last_pad = Some(event.index);

        TrellisPadTelemetry {
            pad_index: event.index.as_u8(),
            event: match event.kind {
                DriverPadEventKind::Pressed => PadEventKind::Pressed,
                DriverPadEventKind::Released => PadEventKind::Released,
            },
        }
    }
}
