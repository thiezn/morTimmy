use anyhow::{Context, Result, anyhow, bail};
use mortimmy_protocol::messages::{
    ControllerMessage, ControllerResponsePayload, ControllerStatus, ControlMessage, HostMessage,
    RequestId, RequestMessage, RequestPayload, WireMessage,
    commands::ParameterKey,
    telemetry::{ControllerCapabilities, ControllerRole},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{Duration, Instant, timeout};
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
    next_request_id: u16,
}

#[derive(Debug)]
struct ManagedSerialController {
    config: SerialConfig,
    response_timeout: Duration,
    inner: Option<SerialPicoTransport>,
    status: Option<ControllerStatus>,
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
            next_request_id: 1,
        }
    }

    /// Return whether the manager currently has an open, validated serial connection.
    pub fn is_connected(&self) -> bool {
        self.controllers.iter().any(ManagedSerialController::is_connected)
    }

    /// Return the discovered controllers and their last known status snapshots.
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

    /// Send one latest-wins control snapshot over the active serial session.
    pub async fn send_control(&mut self, control: ControlMessage) -> Result<Vec<ControllerMessage>> {
        let mut messages = Vec::new();
        let mut targeted_any = false;
        let mut last_error = None;

        for controller in &mut self.controllers {
            let Some(status) = controller.status else {
                continue;
            };

            if !controller_accepts_control(status) {
                continue;
            }

            targeted_any = true;
            match controller.send_control(control).await {
                Ok(controller_messages) => {
                    update_status_from_messages(controller, &controller_messages);
                    messages.extend(controller_messages);
                }
                Err(error) => {
                    let device_path = controller.device_path().to_string();
                    controller.disconnect();
                    last_error = Some(anyhow!("{}: {error:#}", device_path));
                }
            }
        }

        if !targeted_any {
            return if self.is_connected() {
                Ok(Vec::new())
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

        if self.is_connected() {
            Ok(messages)
        } else {
            Err(last_error.unwrap_or_else(|| anyhow!("serial transport is disconnected")))
        }
    }

    /// Send one correlated request over the active serial session.
    pub async fn send_request(&mut self, request: RequestPayload) -> Result<Vec<ControllerMessage>> {
        let request_id = RequestId(self.next_request_id);
        self.next_request_id = self.next_request_id.wrapping_add(1);
        let mut messages = Vec::new();
        let mut targeted_any = false;
        let mut last_error = None;

        for controller in &mut self.controllers {
            let Some(status) = controller.status else {
                continue;
            };

            if !controller_accepts_request(status, &request) {
                continue;
            }

            targeted_any = true;
            match controller.send_request(request_id, request.clone()).await {
                Ok(controller_messages) => {
                    update_status_from_messages(controller, &controller_messages);
                    messages.extend(controller_messages);
                }
                Err(error) => {
                    let device_path = controller.device_path().to_string();
                    controller.disconnect();
                    last_error = Some(anyhow!("{}: {error:#}", device_path));
                }
            }
        }

        if !targeted_any {
            return if self.is_connected() {
                Ok(Vec::new())
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

        if self.is_connected() {
            Ok(messages)
        } else {
            Err(last_error.unwrap_or_else(|| anyhow!("serial transport is disconnected")))
        }
    }

    /// Drain any controller-originated messages currently available on the active serial sessions.
    pub async fn drain_messages(&mut self, timeout: Duration) -> Result<Vec<ControllerMessage>> {
        let mut messages = Vec::new();
        let mut wait = timeout;

        for controller in &mut self.controllers {
            if !controller.is_connected() {
                continue;
            }

            match controller.drain_messages(wait).await {
                Ok(controller_messages) => {
                    update_status_from_messages(controller, &controller_messages);
                    messages.extend(controller_messages);
                }
                Err(error) => {
                    let device_path = controller.device_path().to_string();
                    controller.disconnect();
                    return Err(anyhow!("{}: {error:#}", device_path));
                }
            }

            wait = Duration::ZERO;
        }

        Ok(messages)
    }
}

impl ManagedSerialController {
    /// Create a disconnected controller slot for `config` and `response_timeout`.
    fn new(config: SerialConfig, response_timeout: Duration) -> Self {
        Self {
            config,
            response_timeout,
            inner: None,
            status: None,
        }
    }

    /// Return the primary device path configured for this controller slot.
    fn device_path(&self) -> &str {
        self.config.primary_device_path()
    }

    /// Return whether this controller slot is connected and has discovered status.
    fn is_connected(&self) -> bool {
        self.status.is_some()
    }

    /// Drop the live transport and forget the last discovered status.
    fn disconnect(&mut self) {
        self.inner = None;
        self.status = None;
    }

    /// Open the serial transport and cache the controller status discovered from it.
    async fn try_connect_and_discover(&mut self) -> Result<()> {
        self.try_connect().await?;
        self.status = Some(self.query_status().await?);
        Ok(())
    }

    /// Open the serial transport if it is not already connected.
    async fn try_connect(&mut self) -> Result<()> {
        if self.inner.is_some() {
            return Ok(());
        }

        let candidate = SerialPicoTransport::open(self.config.clone())?;
        self.inner = Some(candidate);
        Ok(())
    }

    /// Query and return the controller status snapshot from the live serial device.
    async fn query_status(&mut self) -> Result<ControllerStatus> {
        let messages = self
            .send_request(RequestId(0), RequestPayload::GetControllerStatus)
            .await?;

        for message in messages {
            if let ControllerMessage::Response(response) = message
                && let ControllerResponsePayload::ControllerStatus(status) = response.payload
            {
                return Ok(status);
            }
        }

        bail!(
            "missing controller status response during discovery on {}",
            self.device_path()
        )
    }

    /// Send one correlated `request` with `request_id` through this controller slot.
    async fn send_request(
        &mut self,
        request_id: RequestId,
        request: RequestPayload,
    ) -> Result<Vec<ControllerMessage>> {
        let result = {
            let transport = self
                .inner
                .as_mut()
                .context("serial transport is disconnected")?;
            transport.send_request(request_id, request, self.response_timeout).await
        };

        if result.is_err() {
            self.disconnect();
        }

        result
    }

    /// Send one latest-wins `control` snapshot through this controller slot.
    async fn send_control(&mut self, control: ControlMessage) -> Result<Vec<ControllerMessage>> {
        let result = {
            let transport = self
                .inner
                .as_mut()
                .context("serial transport is disconnected")?;
            transport.send_control(control, Duration::ZERO).await
        };

        if result.is_err() {
            self.disconnect();
        }

        result
    }

    /// Drain unsolicited controller messages from this controller slot for up to `timeout`.
    async fn drain_messages(&mut self, timeout: Duration) -> Result<Vec<ControllerMessage>> {
        let result = {
            let transport = self
                .inner
                .as_mut()
                .context("serial transport is disconnected")?;
            transport.drain_messages(timeout).await
        };

        if result.is_err() {
            self.disconnect();
        }

        result
    }
}

/// Return whether `status` should receive latest-wins control snapshots.
fn controller_accepts_control(status: ControllerStatus) -> bool {
    status.capabilities.contains(ControllerCapabilities::DRIVE)
        || status.capabilities.contains(ControllerCapabilities::SERVO)
}

/// Return whether `status` should receive the correlated `request`.
fn controller_accepts_request(status: ControllerStatus, request: &RequestPayload) -> bool {
    match request {
        RequestPayload::GetControllerStatus | RequestPayload::ConfigureReports(_) => true,
        RequestPayload::SetParam(update) => match update.key {
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
        RequestPayload::PlayAudio(_) => {
            status.capabilities.contains(ControllerCapabilities::AUDIO_OUTPUT)
        }
        RequestPayload::SetTrellisLeds(_) => {
            status.capabilities.contains(ControllerCapabilities::TEXT_DISPLAY)
        }
    }
}

/// Update the cached controller status when `messages` contains a status response.
fn update_status_from_messages(
    controller: &mut ManagedSerialController,
    messages: &[ControllerMessage],
) {
    for message in messages {
        if let ControllerMessage::Response(response) = message
            && let ControllerResponsePayload::ControllerStatus(status) = response.payload
        {
            controller.status = Some(status);
        }
    }
}

/// Return the first duplicate controller role together with the conflicting device paths.
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

    /// Send one latest-wins control snapshot and drain any immediately available controller messages.
    pub async fn send_control(
        &mut self,
        control: ControlMessage,
        drain_timeout: Duration,
    ) -> Result<Vec<ControllerMessage>> {
        self.write_host_message(&HostMessage::Control(control)).await?;
        self.drain_messages(drain_timeout).await
    }

    /// Send one correlated request and collect messages until the matching response arrives.
    pub async fn send_request(
        &mut self,
        request_id: RequestId,
        request: RequestPayload,
        response_timeout: Duration,
    ) -> Result<Vec<ControllerMessage>> {
        self.write_host_message(&HostMessage::Request(RequestMessage { request_id, payload: request }))
            .await?;
        self.read_until_response(request_id, response_timeout).await
    }

    /// Drain any controller-originated messages available within the provided timeout.
    pub async fn drain_messages(&mut self, timeout_window: Duration) -> Result<Vec<ControllerMessage>> {
        let mut messages = Vec::new();
        let mut wait = timeout_window;

        loop {
            match timeout(wait, self.read_one_controller_message()).await {
                Ok(Ok(message)) => {
                    messages.push(message);
                    wait = Duration::ZERO;
                }
                Ok(Err(error)) => return Err(error),
                Err(_) => return Ok(messages),
            }
        }
    }

    /// Encode and write one host `message` to the serial port.
    async fn write_host_message(&mut self, message: &HostMessage) -> Result<()> {
        let outbound = self.bridge.encode_host_message(message)?;
        self.port
            .write_all(&outbound)
            .await
            .context("failed to write command bytes to the Pico serial link")?;
        self.port
            .flush()
            .await
            .context("failed to flush command bytes to the Pico serial link")?;

        Ok(())
    }

    /// Read controller messages until the response for `request_id` arrives or `response_timeout` expires.
    async fn read_until_response(
        &mut self,
        request_id: RequestId,
        response_timeout: Duration,
    ) -> Result<Vec<ControllerMessage>> {
        let deadline = Instant::now() + response_timeout;
        let mut messages = Vec::new();

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let message = timeout(remaining, self.read_one_controller_message())
                .await
                .context("timed out waiting for a controller response from the Pico serial link")??;
            let is_match = matches!(
                message,
                ControllerMessage::Response(ref response) if response.request_id == request_id
            );
            messages.push(message);

            if is_match {
                return Ok(messages);
            }
        }
    }

    /// Read and decode the next controller message from the serial port.
    async fn read_one_controller_message(&mut self) -> Result<ControllerMessage> {
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
                if let Some(message) = self.bridge.push_rx_byte_lossy(*byte)? {
                    return match message {
                        WireMessage::Controller(message) => Ok(message),
                        WireMessage::Host(_) => bail!("serial transport received a host message from the Pico side"),
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
        ControllerStatus, RequestPayload,
        commands::{ParameterKey, ParameterUpdate},
        telemetry::{ControllerCapabilities, ControllerRole},
    };

    use super::{controller_accepts_control, controller_accepts_request, duplicate_controller_role};
    use crate::brain::transport::ConnectedController;

    fn controller(device_path: &str, controller_role: ControllerRole, capabilities: ControllerCapabilities) -> ConnectedController {
        ConnectedController {
            device_path: device_path.to_string(),
            status: ControllerStatus {
                mode: Mode::Teleop,
                controller_role,
                capabilities,
                uptime_ms: 0,
                link_quality: 100,
                error: None,
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
            ControllerCapabilities::AUDIO_OUTPUT,
        );
        assert!(controller_accepts_control(motion.status));
        assert!(!controller_accepts_control(audio.status));
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
        let audio_param = RequestPayload::SetParam(ParameterUpdate {
            key: ParameterKey::AudioChunkSamples,
            value: 240,
        });
        let timeout_param = RequestPayload::SetParam(ParameterUpdate {
            key: ParameterKey::LinkTimeoutMs,
            value: 500,
        });

        assert!(!controller_accepts_request(motion.status, &audio_param));
        assert!(controller_accepts_request(audio.status, &audio_param));
        assert!(controller_accepts_request(motion.status, &timeout_param));
        assert!(controller_accepts_request(audio.status, &timeout_param));
    }

    #[test]
    fn display_parameters_route_to_motion_controller_only() {
        let motion = controller(
            "/dev/ttyUSB0",
            ControllerRole::MotionController,
            ControllerCapabilities::TEXT_DISPLAY,
        );
        let audio = controller(
            "/dev/ttyUSB1",
            ControllerRole::AudioController,
            ControllerCapabilities::AUDIO_OUTPUT,
        );
        let display_param = RequestPayload::SetParam(ParameterUpdate {
            key: ParameterKey::TrellisBrightness,
            value: 32,
        });

        assert!(controller_accepts_request(motion.status, &display_param));
        assert!(!controller_accepts_request(audio.status, &display_param));
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
