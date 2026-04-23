//! Transport backends used by the host brain.

mod loopback;
mod serial;

use anyhow::Result;
use clap::ValueEnum;
use mortimmy_protocol::messages::{
    command::Command,
    telemetry::{StatusTelemetry, Telemetry},
};
use tokio::time::Duration;

use crate::serial::SerialConfig;

pub use self::loopback::LoopbackPicoTransport;
pub use self::serial::ManagedSerialPicoTransport;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConnectedController {
    pub device_path: String,
    pub status: StatusTelemetry,
}

/// Selects which protocol transport backend the host brain uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TransportBackendKind {
    /// Exchange framed protocol packets with an in-process firmware scaffold.
    Loopback,
    /// Exchange framed protocol packets with a live Pico USB CDC serial device.
    Serial,
}

impl Default for TransportBackendKind {
    fn default() -> Self {
        Self::Serial
    }
}

/// Host-side transport abstraction for protocol commands and telemetry.
#[derive(Debug)]
pub enum BrainTransport {
    Loopback(Box<LoopbackPicoTransport>),
    Serial(Box<ManagedSerialPicoTransport>),
}

impl BrainTransport {
    /// Construct a transport backend from CLI selection and serial configuration.
    pub fn from_kind(kind: TransportBackendKind, serial_config: SerialConfig, response_timeout: Duration) -> Result<Self> {
        match kind {
            TransportBackendKind::Loopback => Ok(Self::Loopback(Box::new(LoopbackPicoTransport::new(serial_config)))),
            TransportBackendKind::Serial => {
                Ok(Self::Serial(Box::new(ManagedSerialPicoTransport::new(serial_config, response_timeout))))
            }
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

    /// Send one protocol command and return the first telemetry response, if any.
    pub async fn exchange_command(&mut self, command: Command) -> Result<Option<Telemetry>> {
        match self {
            Self::Loopback(transport) => transport.exchange_command(command),
            Self::Serial(transport) => transport.exchange_command(command).await,
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
}
