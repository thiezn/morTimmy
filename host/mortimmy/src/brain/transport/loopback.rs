use anyhow::{Result, anyhow, bail};
use mortimmy_protocol::{
    FrameDecoder, MAX_FRAME_BODY_LEN, MAX_PAYLOAD_LEN, decode_message, encode_message, wrap_payload,
};
use mortimmy_protocol::messages::{
    ControllerMessage, ControllerStatus, ControlMessage, HostMessage, RequestId,
    RequestMessage, RequestPayload, WireMessage,
};
use mortimmy_rp2350::FirmwareScaffold;

use crate::serial::{SerialBridge, SerialBridgeError, SerialConfig};

/// Loopback transport that exchanges framed bytes with an in-process firmware scaffold.
#[derive(Debug)]
pub struct LoopbackPicoTransport {
    host_bridge: SerialBridge,
    device: SimulatedPico,
    next_request_id: u16,
}

impl LoopbackPicoTransport {
    /// Create a loopback transport using the same serial framing code as the live host path.
    pub fn new(serial_config: SerialConfig) -> Self {
        Self {
            host_bridge: SerialBridge::new(serial_config),
            device: SimulatedPico::default(),
            next_request_id: 1,
        }
    }

    /// Send one latest-wins control snapshot to the simulated Pico.
    pub fn send_control(&mut self, control: ControlMessage) -> Result<Vec<ControllerMessage>> {
        self.exchange_host_message(HostMessage::Control(control))
    }

    /// Send one correlated request to the simulated Pico.
    pub fn send_request(&mut self, request: RequestPayload) -> Result<Vec<ControllerMessage>> {
        let request_id = RequestId(self.next_request_id);
        self.next_request_id = self.next_request_id.wrapping_add(1);
        self.exchange_host_message(HostMessage::Request(RequestMessage { request_id, payload: request }))
    }

    /// Drain controller-originated messages currently available on the loopback transport.
    pub fn drain_messages(&mut self, _timeout: tokio::time::Duration) -> Result<Vec<ControllerMessage>> {
        Ok(Vec::new())
    }

    /// Exchange one host-originated `message` with the simulated Pico and decode all replies.
    fn exchange_host_message(&mut self, message: HostMessage) -> Result<Vec<ControllerMessage>> {
        let outbound = self.host_bridge.encode_host_message(&message)?;
        let response_bytes = self.device.handle_host_bytes(&outbound)?;
        let mut decoded = Vec::new();

        for byte in response_bytes {
            if let Some(message) = self.host_bridge.push_rx_byte(byte)? {
                match message {
                    WireMessage::Controller(message) => decoded.push(message),
                    WireMessage::Host(_) => {
                        bail!("loopback transport received a host message from the firmware side")
                    }
                }
            }
        }

        Ok(decoded)
    }

    /// Return the loopback device path used for logging and tests.
    pub fn device_path(&self) -> &str {
        self.host_bridge.config.primary_device_path()
    }

    /// Return the simulated controller status snapshot.
    pub fn status(&self) -> ControllerStatus {
        self.device.scaffold.controller_status()
    }
}

#[derive(Debug, Default)]
struct SimulatedPico {
    scaffold: FirmwareScaffold,
    decoder: FrameDecoder,
    outbound_sequence: u16,
}

impl SimulatedPico {
    /// Feed framed host `bytes` into the scaffold and return framed controller replies.
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

/// Convert a frame decoder error into the host serial bridge error surface.
fn serial_frame_error(error: mortimmy_protocol::FrameError) -> SerialBridgeError {
    SerialBridgeError::Frame(error)
}
