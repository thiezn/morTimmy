#![allow(dead_code)]

use mortimmy_core::Mode;
use mortimmy_protocol::messages::command::Command;

/// Placeholder receive task state for the USB and serial link.
#[derive(Clone, Copy, Debug, Default)]
pub struct LinkRxTask {
    /// Last requested robot mode from the host.
    pub last_requested_mode: Option<Mode>,
    /// Last audio chunk index accepted from the host.
    pub last_audio_chunk_index: Option<u16>,
    /// Last Trellis LED mask received from the host.
    pub last_trellis_led_mask: Option<u16>,
    /// Last command kind observed on the wire.
    pub last_command_kind: Option<&'static str>,
}

impl LinkRxTask {
    /// Record the most recent command accepted from the host.
    pub fn record_command(&mut self, command: &Command) {
        self.last_command_kind = Some(command.kind());

        match command {
            Command::SetDesiredState(desired_state) => {
                self.last_requested_mode = Some(desired_state.mode)
            }
            Command::PlayAudio(chunk) => self.last_audio_chunk_index = Some(chunk.chunk_index),
            Command::SetTrellisLeds(command) => self.last_trellis_led_mask = Some(command.led_mask),
            Command::SetParam(_) | Command::GetStatus => {}
        }
    }
}
