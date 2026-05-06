#![allow(dead_code)]

use mortimmy_core::Mode;
use mortimmy_protocol::messages::{HostMessage, RequestPayload};

/// Placeholder receive task state for the USB and serial link.
#[derive(Clone, Copy, Debug, Default)]
pub struct LinkRxTask {
    /// Last requested robot mode from the host.
    pub last_requested_mode: Option<Mode>,
    /// Last audio chunk index accepted from the host.
    pub last_audio_chunk_index: Option<u16>,
    /// Last Trellis LED mask received from the host.
    pub last_trellis_led_mask: Option<u16>,
    /// Last host message kind observed on the wire.
    pub last_message_kind: Option<&'static str>,
}

impl LinkRxTask {
    /// Record the most recent host message accepted from the wire.
    pub fn record_message(&mut self, message: &HostMessage) {
        self.last_message_kind = Some(message.kind());

        match message {
            HostMessage::Control(control) => {
                self.last_requested_mode = Some(control.desired_state.mode);
            }
            HostMessage::Request(request) => match &request.payload {
                RequestPayload::PlayAudio(chunk) => {
                    self.last_audio_chunk_index = Some(chunk.chunk_index);
                }
                RequestPayload::SetTrellisLeds(command) => {
                    self.last_trellis_led_mask = Some(command.led_mask);
                }
                RequestPayload::GetControllerStatus
                | RequestPayload::SetParam(_)
                | RequestPayload::ConfigureReports(_) => {}
            },
        }
    }
}
