//! Transport backends used by the host brain.

mod loopback;
mod serial;

use anyhow::Result;
use clap::ValueEnum;
use mortimmy_protocol::messages::{
    ControllerMessage, ControllerStatus, ControlMessage, RequestPayload,
};
use tokio::time::Duration;

use crate::serial::SerialConfig;

pub use self::loopback::LoopbackPicoTransport;
pub use self::serial::ManagedSerialPicoTransport;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectedController {
    pub device_path: String,
    pub status: ControllerStatus,
}

/// Selects which protocol transport backend the host brain uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum TransportBackendKind {
    /// Exchange framed protocol packets with an in-process firmware scaffold.
    ///
    /// Useful for development and testing without a physical robot, but does
    /// not simulate any real-world conditions.
    Loopback,
    /// Exchange framed protocol packets with a live Pico USB CDC serial device.
    #[default]
    Serial,
}

/// Host-side transport abstraction for protocol commands and telemetry.
#[derive(Debug)]
pub enum BrainTransport {
    Loopback(Box<LoopbackPicoTransport>),
    Serial(Box<ManagedSerialPicoTransport>),
}

impl BrainTransport {
    /// Construct a transport backend from CLI selection and serial configuration.
    pub fn from_kind(
        kind: TransportBackendKind,
        serial_config: SerialConfig,
        response_timeout: Duration,
    ) -> Result<Self> {
        match kind {
            TransportBackendKind::Loopback => Ok(Self::Loopback(Box::new(
                LoopbackPicoTransport::new(serial_config),
            ))),
            TransportBackendKind::Serial => Ok(Self::Serial(Box::new(
                ManagedSerialPicoTransport::new(serial_config, response_timeout),
            ))),
        }
    }

    /// Return whether the active backend currently has a working connection.
    pub fn is_connected(&self) -> bool {
        match self {
            Self::Loopback(_) => true,
            Self::Serial(transport) => transport.is_connected(),
        }
    }

    /// Attempt to establish or re-establish the active backend connection.
    pub async fn try_connect(&mut self) -> Result<()> {
        match self {
            Self::Loopback(_) => Ok(()),
            Self::Serial(transport) => transport.try_connect().await,
        }
    }

    /// Drop any active backend connection.
    pub fn disconnect(&mut self) {
        match self {
            Self::Loopback(_) => {}
            Self::Serial(transport) => transport.disconnect(),
        }
    }

    /// Send one latest-wins control snapshot.
    pub async fn send_control(&mut self, control: ControlMessage) -> Result<Vec<ControllerMessage>> {
        match self {
            Self::Loopback(transport) => transport.send_control(control),
            Self::Serial(transport) => transport.send_control(control).await,
        }
    }

    /// Send one correlated request to all matching controllers.
    pub async fn send_request(&mut self, request: RequestPayload) -> Result<Vec<ControllerMessage>> {
        match self {
            Self::Loopback(transport) => transport.send_request(request),
            Self::Serial(transport) => transport.send_request(request).await,
        }
    }

    /// Drain any controller-originated messages currently available on the transport.
    pub async fn drain_messages(&mut self, timeout: Duration) -> Result<Vec<ControllerMessage>> {
        match self {
            Self::Loopback(transport) => transport.drain_messages(timeout),
            Self::Serial(transport) => transport.drain_messages(timeout).await,
        }
    }

    /// Return the currently discovered controllers that are active on the selected backend.
    pub fn connected_controllers(&self) -> Vec<ConnectedController> {
        match self {
            Self::Loopback(transport) => vec![ConnectedController {
                device_path: transport.device_path().to_string(),
                status: transport.status(),
            }],
            Self::Serial(transport) => transport.connected_controllers(),
        }
    }

    /// Return whether this backend can currently emit unsolicited controller reports/events.
    pub const fn supports_unsolicited_messages(&self) -> bool {
        match self {
            Self::Loopback(_) => false,
            Self::Serial(_) => true,
        }
    }
}
