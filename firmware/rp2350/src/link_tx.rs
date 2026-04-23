#![allow(dead_code)]

use mortimmy_protocol::messages::telemetry::Telemetry;

/// Placeholder transmit task state for the USB and serial link.
#[derive(Clone, Copy, Debug, Default)]
pub struct LinkTxTask {
    /// Outgoing protocol sequence number.
    pub sequence: u16,
    /// Whether audio status telemetry should be emitted.
    pub audio_status_dirty: bool,
    /// Whether Trellis input telemetry should be emitted.
    pub trellis_event_dirty: bool,
    /// Last telemetry kind emitted onto the wire.
    pub last_telemetry_kind: Option<&'static str>,
}

impl LinkTxTask {
    /// Record a telemetry message that is about to be sent to the host.
    pub fn record_telemetry(&mut self, telemetry: &Telemetry) {
        self.sequence = self.sequence.wrapping_add(1);
        self.last_telemetry_kind = Some(telemetry.kind());

        match telemetry {
            Telemetry::AudioStatus(_) => self.audio_status_dirty = false,
            Telemetry::TrellisPad(_) => self.trellis_event_dirty = false,
            _ => {}
        }
    }
}
