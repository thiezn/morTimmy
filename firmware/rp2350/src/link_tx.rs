#![allow(dead_code)]

use mortimmy_protocol::messages::{
    ControllerEvent, ControllerMessage, ReportConfig, ReportKind, ReportMessage, ReportPayload,
    telemetry::{ForwardRangeTelemetry, RangeSensorPosition, RangeTelemetry},
};

const RANGE_PENDING_LEFT: u8 = 1 << 0;
const RANGE_PENDING_RIGHT: u8 = 1 << 1;

/// Placeholder transmit task state for the USB and serial link.
#[derive(Clone, Copy, Debug, Default)]
pub struct LinkTxTask {
    /// Outgoing protocol sequence number.
    pub sequence: u16,
    /// Whether audio status telemetry should be emitted.
    pub audio_status_dirty: bool,
    /// Whether Trellis input telemetry should be emitted.
    pub trellis_event_dirty: bool,
    /// Host-configured cadence for range reports.
    pub range_report_config: Option<ReportConfig>,
    /// Pending forward range samples that should be emitted.
    pub range_pending_mask: u8,
    /// Timestamp of the last emitted range report in milliseconds.
    pub last_range_report_at_ms: u64,
    /// Last controller message kind emitted onto the wire.
    pub last_message_kind: Option<&'static str>,
}

impl LinkTxTask {
    /// Record a controller message that is about to be sent to the host.
    pub fn record_message(&mut self, message: &ControllerMessage) {
        self.sequence = self.sequence.wrapping_add(1);
        self.last_message_kind = Some(message.kind());

        match message {
            ControllerMessage::Report(report) => match report.payload {
                ReportPayload::AudioStatus(_) => self.audio_status_dirty = false,
                ReportPayload::Range(range) => {
                    self.range_pending_mask &= !range_pending_bit(range.sensor);
                }
                _ => {}
            },
            ControllerMessage::Event(ControllerEvent::TrellisPad(_)) => {
                self.trellis_event_dirty = false;
            }
            ControllerMessage::Response(_) => {}
        }
    }

    /// Apply host report configuration to the transmit state.
    pub fn configure_report(&mut self, config: ReportConfig, ranges: ForwardRangeTelemetry) {
        match config.report {
            ReportKind::Range => {
                self.range_report_config = Some(config);
                self.range_pending_mask = available_range_mask(ranges);
                self.last_range_report_at_ms = 0;
            }
            ReportKind::AudioStatus => {
                self.audio_status_dirty = true;
            }
            ReportKind::ControlApplied | ReportKind::Battery => {}
        }
    }

    /// Mark a range sensor sample as pending for emission.
    pub fn queue_range_sample(&mut self, sensor: RangeSensorPosition) {
        self.range_pending_mask |= range_pending_bit(sensor);
    }

    /// Return the next queued background controller message, if any.
    pub fn next_message(
        &mut self,
        now_ms: u64,
        ranges: ForwardRangeTelemetry,
    ) -> Option<ControllerMessage> {
        let Some(config) = self.range_report_config else {
            return None;
        };

        self.range_pending_mask &= available_range_mask(ranges);

        if !config.emit_on_change && self.range_pending_mask == 0 {
            self.range_pending_mask = available_range_mask(ranges);
        }

        if self.range_pending_mask == 0 {
            return None;
        }

        let min_interval_ms = u64::from(config.min_interval_ms.max(1));
        if now_ms.saturating_sub(self.last_range_report_at_ms) < min_interval_ms {
            return None;
        }

        let range = next_pending_range(self.range_pending_mask, ranges)?;
        self.last_range_report_at_ms = now_ms;
        Some(ControllerMessage::Report(ReportMessage {
            payload: ReportPayload::Range(range),
        }))
    }
}

/// Return the pending-bit mask for `sensor`.
const fn range_pending_bit(sensor: RangeSensorPosition) -> u8 {
    match sensor {
        RangeSensorPosition::ForwardLeft => RANGE_PENDING_LEFT,
        RangeSensorPosition::ForwardRight => RANGE_PENDING_RIGHT,
    }
}

/// Return a pending-bit mask for the range samples currently present in `ranges`.
fn available_range_mask(ranges: ForwardRangeTelemetry) -> u8 {
    let mut mask = 0;

    if ranges.forward_left.is_some() {
        mask |= RANGE_PENDING_LEFT;
    }
    if ranges.forward_right.is_some() {
        mask |= RANGE_PENDING_RIGHT;
    }

    mask
}

/// Return the next pending range sample selected by `mask` from `ranges`.
fn next_pending_range(mask: u8, ranges: ForwardRangeTelemetry) -> Option<RangeTelemetry> {
    if mask & RANGE_PENDING_LEFT != 0 {
        return ranges.forward_left;
    }

    if mask & RANGE_PENDING_RIGHT != 0 {
        return ranges.forward_right;
    }

    None
}
