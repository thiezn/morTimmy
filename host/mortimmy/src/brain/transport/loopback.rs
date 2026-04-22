use anyhow::{Result, anyhow, bail};
use mortimmy_protocol::{
    FrameDecoder, MAX_FRAME_BODY_LEN, MAX_PAYLOAD_LEN, decode_message, encode_message, wrap_payload,
};
use mortimmy_protocol::messages::{Command, Telemetry, WireMessage};
use mortimmy_rp2350::FirmwareScaffold;

use crate::serial::{SerialBridge, SerialBridgeError, SerialConfig};

/// Loopback transport that exchanges framed bytes with an in-process firmware scaffold.
#[derive(Debug)]
pub struct LoopbackPicoTransport {
    host_bridge: SerialBridge,
    device: SimulatedPico,
}

impl LoopbackPicoTransport {
    /// Create a loopback transport using the same serial framing code as the live host path.
    pub fn new(serial_config: SerialConfig) -> Self {
        Self {
            host_bridge: SerialBridge::new(serial_config),
            device: SimulatedPico::default(),
        }
    }

    /// Exchange one command with the simulated Pico and decode the first telemetry response.
    pub fn exchange_command(&mut self, command: Command) -> Result<Option<Telemetry>> {
        let outbound = self.host_bridge.encode_command(command)?;
        let response_bytes = self.device.handle_host_bytes(&outbound)?;
        let mut decoded = None;

        for byte in response_bytes {
            if let Some(message) = self.host_bridge.push_rx_byte(byte)? {
                decoded = Some(message);
            }
        }

        match decoded {
            Some(WireMessage::Telemetry(telemetry)) => Ok(Some(telemetry)),
            Some(WireMessage::Command(_)) => bail!("loopback transport received a command from the firmware side"),
            None => Ok(None),
        }
    }
}

#[derive(Debug, Default)]
struct SimulatedPico {
    scaffold: FirmwareScaffold,
    decoder: FrameDecoder,
    outbound_sequence: u16,
}

impl SimulatedPico {
    fn handle_host_bytes(&mut self, bytes: &[u8]) -> Result<Vec<u8>> {
        let mut encoded_responses = Vec::new();

        for byte in bytes {
            if let Some(frame) = self.decoder.push(*byte).map_err(serial_frame_error)? {
                let message = decode_message(frame.payload.as_slice())
                    .map_err(|error| anyhow!("failed to decode host payload on loopback pico: {error:?}"))?;
                if let Some(response) = self.scaffold.apply_wire_message(message) {
                    let mut payload_buffer = [0u8; MAX_PAYLOAD_LEN];
                    let payload = encode_message(&response, &mut payload_buffer)
                        .map_err(|error| anyhow!("failed to encode firmware response on loopback pico: {error:?}"))?;
                    let mut frame_buffer = [0u8; MAX_FRAME_BODY_LEN + 1];
                    let encoded = wrap_payload(payload, self.outbound_sequence, &mut frame_buffer)
                        .map_err(|error| anyhow!("failed to frame firmware response on loopback pico: {error:?}"))?;
                    self.outbound_sequence = self.outbound_sequence.wrapping_add(1);
                    encoded_responses.extend_from_slice(encoded);
                }
            }
        }

        Ok(encoded_responses)
    }
}

fn serial_frame_error(error: mortimmy_protocol::FrameError) -> SerialBridgeError {
    SerialBridgeError::Frame(error)
}
