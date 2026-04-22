#![allow(dead_code)]

//! Host-side serial framing helpers built on the shared protocol crate.

use std::fmt;

use mortimmy_protocol::{
    CodecError, FrameDecoder, FrameError, MAX_FRAME_BODY_LEN, MAX_PAYLOAD_LEN, decode_message,
    encode_message, wrap_payload,
};
use mortimmy_protocol::messages::{Command, WireMessage};
use serde::{Deserialize, Serialize};

/// Errors returned while encoding or decoding framed serial traffic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SerialBridgeError {
    Codec(CodecError),
    Frame(FrameError),
}

impl fmt::Display for SerialBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(error) => write!(f, "serial codec error: {error:?}"),
            Self::Frame(error) => write!(f, "serial frame error: {error:?}"),
        }
    }
}

impl std::error::Error for SerialBridgeError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SerialConfig {
    pub device_path: String,
    pub baud_rate: u32,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            device_path: "/dev/ttyACM0".to_string(),
            baud_rate: 115_200,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SerialBridge {
    pub config: SerialConfig,
    outbound_sequence: u16,
    decoder: FrameDecoder,
}

impl SerialBridge {
    /// Create a serial bridge with an empty decoder and sequence counter.
    pub fn new(config: SerialConfig) -> Self {
        Self {
            config,
            outbound_sequence: 0,
            decoder: FrameDecoder::default(),
        }
    }

    /// Return the next outbound sequence number that will be used.
    pub const fn outbound_sequence(&self) -> u16 {
        self.outbound_sequence
    }

    /// Encode a concrete control command into a framed byte buffer.
    pub fn encode_command(&mut self, command: Command) -> Result<Vec<u8>, SerialBridgeError> {
        self.encode_wire_message(&WireMessage::Command(command))
    }

    /// Encode a wire message into a COBS-framed transport packet.
    pub fn encode_wire_message(&mut self, message: &WireMessage) -> Result<Vec<u8>, SerialBridgeError> {
        let mut payload_buffer = [0u8; MAX_PAYLOAD_LEN];
        let payload = encode_message(message, &mut payload_buffer).map_err(SerialBridgeError::Codec)?;
        let mut frame_buffer = [0u8; MAX_FRAME_BODY_LEN + 1];
        let frame = wrap_payload(payload, self.outbound_sequence, &mut frame_buffer).map_err(SerialBridgeError::Frame)?;
        self.outbound_sequence = self.outbound_sequence.wrapping_add(1);
        Ok(frame.to_vec())
    }

    /// Feed one received byte into the decoder and emit a complete wire message when available.
    pub fn push_rx_byte(&mut self, byte: u8) -> Result<Option<WireMessage>, SerialBridgeError> {
        match self.decoder.push(byte).map_err(SerialBridgeError::Frame)? {
            Some(frame) => decode_message(frame.payload.as_slice())
                .map(Some)
                .map_err(SerialBridgeError::Codec),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use mortimmy_core::{Mode, PwmTicks, ServoTicks};
    use mortimmy_protocol::{FrameDecoder, decode_message, wrap_payload};
    use mortimmy_protocol::messages::{Command, DesiredStateCommand, DriveCommand, ServoCommand, StatusTelemetry, Telemetry, WireMessage};

    use super::{SerialBridge, SerialConfig};

    #[test]
    fn encodes_desired_state_command_into_protocol_frame() {
        let mut bridge = SerialBridge::new(SerialConfig::default());
        let bytes = bridge
            .encode_command(Command::SetDesiredState(DesiredStateCommand::new(
                Mode::Teleop,
                DriveCommand {
                    left: PwmTicks(300),
                    right: PwmTicks(-150),
                },
                ServoCommand {
                    pan: ServoTicks(0),
                    tilt: ServoTicks(0),
                },
            )))
            .unwrap();
        let mut decoder = FrameDecoder::default();
        let mut decoded = None;

        for byte in bytes {
            if let Some(frame) = decoder.push(byte).unwrap() {
                assert_eq!(frame.sequence, 0);
                decoded = Some(decode_message(frame.payload.as_slice()).unwrap());
            }
        }

        assert_eq!(bridge.outbound_sequence(), 1);
        assert_eq!(
            decoded,
            Some(WireMessage::Command(Command::SetDesiredState(DesiredStateCommand::new(
                Mode::Teleop,
                DriveCommand {
                    left: PwmTicks(300),
                    right: PwmTicks(-150),
                },
                ServoCommand {
                    pan: ServoTicks(0),
                    tilt: ServoTicks(0),
                },
            ))))
        );
    }

    #[test]
    fn decodes_telemetry_stream() {
        let mut bridge = SerialBridge::new(SerialConfig::default());
        let message = WireMessage::Telemetry(Telemetry::Status(StatusTelemetry {
            mode: Mode::Idle,
            uptime_ms: 42,
            link_quality: 100,
            error: None,
        }));
        let mut payload_buffer = [0u8; mortimmy_protocol::MAX_PAYLOAD_LEN];
        let payload = mortimmy_protocol::encode_message(&message, &mut payload_buffer).unwrap();
        let mut frame_buffer = [0u8; mortimmy_protocol::MAX_FRAME_BODY_LEN + 1];
        let frame = wrap_payload(payload, 7, &mut frame_buffer).unwrap();
        let mut decoded = None;

        for byte in frame {
            if let Some(message) = bridge.push_rx_byte(*byte).unwrap() {
                decoded = Some(message);
            }
        }

        assert_eq!(decoded, Some(message));
    }
}
