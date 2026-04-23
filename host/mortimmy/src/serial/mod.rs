//! Host-side serial framing helpers built on the shared protocol crate.

use std::fmt;

use mortimmy_protocol::{
    CodecError, FrameDecoder, FrameError, MAX_FRAME_BODY_LEN, MAX_PAYLOAD_LEN, decode_message,
    encode_message, wrap_payload,
};
use mortimmy_protocol::messages::{WireMessage, command::Command};
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

fn default_device_paths() -> Vec<String> {
    vec!["/dev/ttyACM0".to_string()]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SerialConfig {
    #[serde(default = "default_device_paths")]
    pub device_paths: Vec<String>,
    pub baud_rate: u32,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            device_paths: default_device_paths(),
            baud_rate: 115_200,
        }
    }
}

impl SerialConfig {
    pub fn configured_device_paths(&self) -> Vec<String> {
        if !self.device_paths.is_empty() {
            return self.device_paths.clone();
        }

        default_device_paths()
    }

    pub fn display_paths(&self) -> String {
        self.configured_device_paths().join(", ")
    }

    pub fn split_by_device(&self) -> Vec<Self> {
        self.configured_device_paths()
            .into_iter()
            .map(|device_path| Self {
                device_paths: vec![device_path],
                baud_rate: self.baud_rate,
            })
            .collect()
    }

    pub fn primary_device_path(&self) -> &str {
        self.device_paths
            .first()
            .map(String::as_str)
            .unwrap_or("/dev/ttyACM0")
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
    #[cfg(test)]
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
    use mortimmy_protocol::messages::{
        WireMessage,
        command::Command,
        commands::{DesiredStateCommand, DriveCommand, ServoCommand},
        telemetry::{ControllerCapabilities, ControllerRole, StatusTelemetry, Telemetry},
    };

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
            mode: Mode::Teleop,
            controller_role: ControllerRole::MotionController,
            capabilities: ControllerCapabilities::DRIVE,
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

    #[test]
    fn serial_config_supports_multiple_device_paths() {
        let config = SerialConfig {
            device_paths: vec!["/dev/ttyUSB0".to_string(), "/dev/ttyUSB1".to_string()],
            baud_rate: 230_400,
        };

        assert_eq!(config.configured_device_paths(), vec!["/dev/ttyUSB0", "/dev/ttyUSB1"]);
        assert_eq!(config.display_paths(), "/dev/ttyUSB0, /dev/ttyUSB1");
        assert_eq!(
            config
                .split_by_device()
                .into_iter()
                .map(|config| config.primary_device_path().to_string())
                .collect::<Vec<_>>(),
            vec!["/dev/ttyUSB0", "/dev/ttyUSB1"]
        );
    }

    #[test]
    fn serial_config_rejects_legacy_single_device_field() {
        let error = toml::from_str::<SerialConfig>(
            "device_path = \"/dev/ttyUSB9\"\nbaud_rate = 115200\n",
        )
        .unwrap_err();

        assert!(error.to_string().contains("unknown field `device_path`"));
    }
}
