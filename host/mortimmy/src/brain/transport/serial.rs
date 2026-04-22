use anyhow::{Context, Result, anyhow, bail};
use mortimmy_protocol::messages::{Command, Telemetry, WireMessage};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{Duration, timeout};
use tokio_serial::SerialStream;

use crate::serial::{SerialBridge, SerialConfig};

const READ_BUFFER_LEN: usize = 256;

/// Serial transport manager that can disconnect and reconnect around a live Pico CDC session.
#[derive(Debug)]
pub struct ManagedSerialPicoTransport {
    config: SerialConfig,
    response_timeout: Duration,
    inner: Option<SerialPicoTransport>,
}

impl ManagedSerialPicoTransport {
    /// Construct a disconnected manager for a live Pico USB CDC device.
    pub fn new(config: SerialConfig, response_timeout: Duration) -> Self {
        Self {
            config,
            response_timeout,
            inner: None,
        }
    }

    /// Return whether the manager currently has an open, validated serial connection.
    pub const fn is_connected(&self) -> bool {
        self.inner.is_some()
    }

    /// Drop any active serial connection and mark the transport disconnected.
    pub fn disconnect(&mut self) {
        self.inner = None;
    }

    /// Attempt to open the configured serial device.
    pub async fn try_connect(&mut self) -> Result<()> {
        if self.inner.is_some() {
            return Ok(());
        }

        let candidate = SerialPicoTransport::open(self.config.clone())?;
        self.inner = Some(candidate);
        Ok(())
    }

    /// Exchange one command over the active serial session.
    pub async fn exchange_command(&mut self, command: Command) -> Result<Option<Telemetry>> {
        let result = {
            let transport = self
                .inner
                .as_mut()
                .context("serial transport is disconnected")?;
            transport.exchange_command(command, self.response_timeout).await
        };

        if result.is_err() {
            self.disconnect();
        }

        result
    }
}

/// Serial transport that exchanges framed protocol packets with a live Pico USB CDC device.
#[derive(Debug)]
pub struct SerialPicoTransport {
    bridge: SerialBridge,
    port: SerialStream,
}

impl SerialPicoTransport {
    /// Open the configured serial device and prepare the framing bridge.
    pub fn open(config: SerialConfig) -> Result<Self> {
        #[cfg(unix)]
        let builder = tokio_serial::new(&config.device_path, config.baud_rate).exclusive(false);

        #[cfg(not(unix))]
        let builder = tokio_serial::new(&config.device_path, config.baud_rate);

        let port = SerialStream::open(&builder)
            .with_context(|| format!("failed to open serial device {}", config.device_path))?;

        Ok(Self {
            bridge: SerialBridge::new(config),
            port,
        })
    }

    /// Exchange one command with the Pico over a real serial link and decode the first telemetry response.
    pub async fn exchange_command(&mut self, command: Command, response_timeout: Duration) -> Result<Option<Telemetry>> {
        let outbound = self.bridge.encode_command(command)?;
        self.port
            .write_all(&outbound)
            .await
            .context("failed to write command bytes to the Pico serial link")?;
        self.port
            .flush()
            .await
            .context("failed to flush command bytes to the Pico serial link")?;

        timeout(response_timeout, self.read_response())
            .await
            .context("timed out waiting for telemetry from the Pico serial link")?
    }

    async fn read_response(&mut self) -> Result<Option<Telemetry>> {
        let mut read_buffer = [0u8; READ_BUFFER_LEN];

        loop {
            let read = self
                .port
                .read(&mut read_buffer)
                .await
                .context("failed to read bytes from the Pico serial link")?;

            if read == 0 {
                return Err(anyhow!("Pico serial device closed while waiting for telemetry"));
            }

            for byte in &read_buffer[..read] {
                if let Some(message) = self.bridge.push_rx_byte(*byte)? {
                    return match message {
                        WireMessage::Telemetry(telemetry) => Ok(Some(telemetry)),
                        WireMessage::Command(_) => bail!("serial transport received a command from the Pico side"),
                    };
                }
            }
        }
    }
}
