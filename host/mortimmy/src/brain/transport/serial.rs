use anyhow::{Context, Result, anyhow, bail};
use mortimmy_protocol::messages::{
    WireMessage,
    command::Command,
    commands::ParameterKey,
    telemetry::{ControllerCapabilities, ControllerRole, StatusTelemetry, Telemetry},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{Duration, timeout};
use tokio_serial::SerialStream;

use crate::{
    brain::transport::ConnectedController,
    serial::{SerialBridge, SerialConfig},
};

const READ_BUFFER_LEN: usize = 256;

/// Serial transport manager that can disconnect and reconnect around a live Pico CDC session.
#[derive(Debug)]
pub struct ManagedSerialPicoTransport {
    controllers: Vec<ManagedSerialController>,
}

#[derive(Debug)]
struct ManagedSerialController {
    config: SerialConfig,
    response_timeout: Duration,
    inner: Option<SerialPicoTransport>,
    status: Option<StatusTelemetry>,
}

impl ManagedSerialPicoTransport {
    /// Construct a disconnected manager for a live Pico USB CDC device.
    pub fn new(config: SerialConfig, response_timeout: Duration) -> Self {
        Self {
            controllers: config
                .split_by_device()
                .into_iter()
                .map(|config| ManagedSerialController::new(config, response_timeout))
                .collect(),
        }
    }

    /// Return whether the manager currently has an open, validated serial connection.
    pub fn is_connected(&self) -> bool {
        self.controllers.iter().any(ManagedSerialController::is_connected)
    }

    pub fn connected_controllers(&self) -> Vec<ConnectedController> {
        self.controllers
            .iter()
            .filter_map(|controller| {
                controller.status.map(|status| ConnectedController {
                    device_path: controller.device_path().to_string(),
                    status,
                })
            })
            .collect()
    }

    /// Drop any active serial connection and mark the transport disconnected.
    pub fn disconnect(&mut self) {
        for controller in &mut self.controllers {
            controller.disconnect();
        }
    }

    /// Attempt to open the configured serial device.
    pub async fn try_connect(&mut self) -> Result<()> {
        let mut last_error = None;

        for controller in &mut self.controllers {
            if controller.is_connected() {
                continue;
            }

            if let Err(error) = controller.try_connect_and_discover().await {
                last_error = Some(anyhow!("{}: {error:#}", controller.device_path()));
            }
        }

        if let Some((role, device_paths)) = duplicate_controller_role(&self.connected_controllers()) {
            return Err(anyhow!(
                "multiple controllers reported role {:?}: {}",
                role,
                device_paths.join(", ")
            ));
        }

        if self.is_connected() {
            Ok(())
        } else {
            Err(last_error.unwrap_or_else(|| anyhow!("serial transport is disconnected")))
        }
    }

    /// Exchange one command over the active serial session.
    pub async fn exchange_command(&mut self, command: Command) -> Result<Option<Telemetry>> {
        let mut first_telemetry = None;
        let mut targeted_any = false;
        let mut last_error = None;

        for controller in &mut self.controllers {
            let Some(status) = controller.status else {
                continue;
            };

            if !controller_accepts_command(status, &command) {
                continue;
            }

            targeted_any = true;
            match controller.exchange_command(command.clone()).await {
                Ok(Some(Telemetry::Status(status))) => {
                    controller.status = Some(status);
                    if first_telemetry.is_none() {
                        first_telemetry = Some(Telemetry::Status(status));
                    }
                }
                Ok(Some(telemetry)) => {
                    if first_telemetry.is_none() {
                        first_telemetry = Some(telemetry);
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    let device_path = controller.device_path().to_string();
                    controller.disconnect();
                    last_error = Some(anyhow!("{}: {error:#}", device_path));
                }
            }
        }

        if !targeted_any {
            return if self.is_connected() {
                Ok(None)
            } else {
                Err(anyhow!("serial transport is disconnected"))
            };
        }

        if let Some((role, device_paths)) = duplicate_controller_role(&self.connected_controllers()) {
            return Err(anyhow!(
                "multiple controllers reported role {:?}: {}",
                role,
                device_paths.join(", ")
            ));
        }

        if first_telemetry.is_some() {
            return Ok(first_telemetry);
        }

        if self.is_connected() {
            Ok(None)
        } else {
            Err(last_error.unwrap_or_else(|| anyhow!("serial transport is disconnected")))
        }
    }
}

impl ManagedSerialController {
    fn new(config: SerialConfig, response_timeout: Duration) -> Self {
        Self {
            config,
            response_timeout,
            inner: None,
            status: None,
        }
    }

    fn device_path(&self) -> &str {
        self.config.primary_device_path()
    }

    fn is_connected(&self) -> bool {
        self.status.is_some()
    }

    fn disconnect(&mut self) {
        self.inner = None;
        self.status = None;
    }

    async fn try_connect_and_discover(&mut self) -> Result<()> {
        self.try_connect().await?;
        self.status = Some(self.query_status().await?);
        Ok(())
    }

    async fn try_connect(&mut self) -> Result<()> {
        if self.inner.is_some() {
            return Ok(());
        }

        let candidate = SerialPicoTransport::open(self.config.clone())?;
        self.inner = Some(candidate);
        Ok(())
    }

    async fn query_status(&mut self) -> Result<StatusTelemetry> {
        match self.exchange_command(Command::GetStatus).await? {
            Some(Telemetry::Status(status)) => Ok(status),
            Some(telemetry) => bail!(
                "unexpected telemetry during controller discovery on {}: {telemetry:?}",
                self.device_path()
            ),
            None => bail!("missing status telemetry during controller discovery on {}", self.device_path()),
        }
    }

    async fn exchange_command(&mut self, command: Command) -> Result<Option<Telemetry>> {
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

fn controller_accepts_command(status: StatusTelemetry, command: &Command) -> bool {
    match command {
        Command::SetDesiredState(_) => {
            status.capabilities.contains(ControllerCapabilities::DRIVE)
                || status.capabilities.contains(ControllerCapabilities::SERVO)
        }
        Command::SetParam(update) => match update.key {
            ParameterKey::MaxDrivePwm => status.capabilities.contains(ControllerCapabilities::DRIVE),
            ParameterKey::MaxServoStep => status.capabilities.contains(ControllerCapabilities::SERVO),
            ParameterKey::LinkTimeoutMs => true,
            ParameterKey::TrellisBrightness | ParameterKey::TrellisPollIntervalMs => {
                status.capabilities.contains(ControllerCapabilities::TEXT_DISPLAY)
            }
            ParameterKey::AudioChunkSamples => {
                status.capabilities.contains(ControllerCapabilities::AUDIO_OUTPUT)
            }
        },
        Command::PlayAudio(_) => status.capabilities.contains(ControllerCapabilities::AUDIO_OUTPUT),
        Command::SetTrellisLeds(_) => status.capabilities.contains(ControllerCapabilities::TEXT_DISPLAY),
        Command::GetStatus => true,
    }
}

fn duplicate_controller_role(
    controllers: &[ConnectedController],
) -> Option<(ControllerRole, Vec<String>)> {
    for (index, controller) in controllers.iter().enumerate() {
        let mut duplicate_paths = vec![controller.device_path.clone()];
        duplicate_paths.extend(
            controllers[index + 1..]
                .iter()
                .filter(|other| other.status.controller_role == controller.status.controller_role)
                .map(|other| other.device_path.clone()),
        );

        if duplicate_paths.len() > 1 {
            return Some((controller.status.controller_role, duplicate_paths));
        }
    }

    None
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
        let builder = tokio_serial::new(config.primary_device_path(), config.baud_rate).exclusive(false);

        #[cfg(not(unix))]
        let builder = tokio_serial::new(config.primary_device_path(), config.baud_rate);

        let port = SerialStream::open(&builder)
            .with_context(|| format!("failed to open serial device {}", config.primary_device_path()))?;

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

#[cfg(test)]
mod tests {
    use mortimmy_core::Mode;
    use mortimmy_protocol::messages::{
        command::Command,
        commands::{DesiredStateCommand, DriveCommand, ParameterKey, ParameterUpdate, ServoCommand},
        telemetry::{ControllerCapabilities, ControllerRole, StatusTelemetry},
    };

    use super::{controller_accepts_command, duplicate_controller_role};
    use crate::brain::transport::ConnectedController;

    fn controller(device_path: &str, controller_role: ControllerRole, capabilities: ControllerCapabilities) -> ConnectedController {
        ConnectedController {
            device_path: device_path.to_string(),
            status: StatusTelemetry {
                mode: Mode::Teleop,
                controller_role,
                capabilities,
                uptime_ms: 0,
                link_quality: 100,
                error: None,
                range: None,
            },
        }
    }

    #[test]
    fn desired_state_routes_only_to_motion_capabilities() {
        let motion = controller(
            "/dev/ttyUSB0",
            ControllerRole::MotionController,
            ControllerCapabilities::DRIVE.union(ControllerCapabilities::SERVO),
        );
        let audio = controller(
            "/dev/ttyUSB1",
            ControllerRole::AudioController,
            ControllerCapabilities::AUDIO_OUTPUT.union(ControllerCapabilities::TEXT_DISPLAY),
        );
        let command = Command::SetDesiredState(DesiredStateCommand::new(
            Mode::Teleop,
            DriveCommand {
                left: mortimmy_core::PwmTicks(0),
                right: mortimmy_core::PwmTicks(0),
            },
            ServoCommand {
                pan: mortimmy_core::ServoTicks(0),
                tilt: mortimmy_core::ServoTicks(0),
            },
        ));

        assert!(controller_accepts_command(motion.status, &command));
        assert!(!controller_accepts_command(audio.status, &command));
    }

    #[test]
    fn capability_specific_parameters_route_to_matching_controller_only() {
        let motion = controller(
            "/dev/ttyUSB0",
            ControllerRole::MotionController,
            ControllerCapabilities::DRIVE,
        );
        let audio = controller(
            "/dev/ttyUSB1",
            ControllerRole::AudioController,
            ControllerCapabilities::AUDIO_OUTPUT,
        );
        let audio_param = Command::SetParam(ParameterUpdate {
            key: ParameterKey::AudioChunkSamples,
            value: 240,
        });
        let timeout_param = Command::SetParam(ParameterUpdate {
            key: ParameterKey::LinkTimeoutMs,
            value: 500,
        });

        assert!(!controller_accepts_command(motion.status, &audio_param));
        assert!(controller_accepts_command(audio.status, &audio_param));
        assert!(controller_accepts_command(motion.status, &timeout_param));
        assert!(controller_accepts_command(audio.status, &timeout_param));
    }

    #[test]
    fn duplicate_roles_are_detected() {
        let duplicates = vec![
            controller(
                "/dev/ttyUSB0",
                ControllerRole::MotionController,
                ControllerCapabilities::DRIVE,
            ),
            controller(
                "/dev/ttyUSB1",
                ControllerRole::MotionController,
                ControllerCapabilities::DRIVE,
            ),
        ];

        let duplicate = duplicate_controller_role(&duplicates).unwrap();

        assert_eq!(duplicate.0, ControllerRole::MotionController);
        assert_eq!(duplicate.1, vec!["/dev/ttyUSB0", "/dev/ttyUSB1"]);
    }
}
