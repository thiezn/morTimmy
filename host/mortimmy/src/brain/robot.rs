use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use mortimmy_core::{Mode, ServoTicks};
use mortimmy_protocol::messages::{command::Command, commands::ServoCommand, telemetry::{RangeTelemetry, Telemetry}};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time::{Duration, Instant};

use crate::{
    brain::autonomy::{AutonomousTarget, AutonomyRunner},
    config::{LogLevel, NexoConfig, SessionConfig},
    input::{CommandInputSource, ControlState, ControllerId, ControllerInfo, InputEvent},
    nexo::NexoGateway,
    telemetry::TelemetryFanout,
    tui::SessionOutput,
};

use super::{BrainCommand, command_mapping::RouterPolicy, transport::BrainTransport};

const ACTIVE_DESIRED_STATE_INTERVAL: Duration = Duration::from_millis(75);
const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(10);
const COMMAND_COMPLETION_HOLD_GRACE: Duration = Duration::from_millis(750);
const MIN_LINK_TIMEOUT: Duration = Duration::from_millis(250);

/// Result of handling a single operator command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BrainStep {
    Continue(Option<Telemetry>),
    Exit,
}

/// Main host-side control loop.
#[derive(Debug)]
pub struct RobotBrain {
    router: RouterPolicy,
    transport: BrainTransport,
    telemetry: TelemetryFanout,
    nexo: NexoGateway,
    chat_event_tx: UnboundedSender<(LogLevel, String)>,
    chat_event_rx: UnboundedReceiver<(LogLevel, String)>,
    autonomy: AutonomyRunner,
    active_controllers: BTreeMap<ControllerId, ControllerInfo>,
    desired_mode: Mode,
    desired_servo: ServoCommand,
    active_control_state: ControlState,
    health_check_interval: Duration,
    reconnect_interval: Duration,
    active_desired_state_interval: Duration,
    input_poll_interval: Duration,
    last_contact_at: Instant,
    last_desired_state_at: Option<Instant>,
    applied_link_timeout_ms: Option<u32>,
}

impl RobotBrain {
    /// Construct the robot brain from a router policy, transport, and telemetry fanout.
    #[cfg(test)]
    pub fn new(
        router: RouterPolicy,
        transport: BrainTransport,
        telemetry: TelemetryFanout,
        session: SessionConfig,
    ) -> Self {
        Self::with_nexo(router, transport, telemetry, session, NexoConfig::default())
    }

    pub fn with_nexo(
        router: RouterPolicy,
        transport: BrainTransport,
        telemetry: TelemetryFanout,
        session: SessionConfig,
        nexo_config: NexoConfig,
    ) -> Self {
        let now = Instant::now();
        let desired_mode = router.default_mode;
        let desired_servo = RouterPolicy::centered_servo();
        let reconnect_interval = Duration::from_millis(session.reconnect_interval_ms.max(1));
        let (chat_event_tx, chat_event_rx) = mpsc::unbounded_channel();
        Self {
            router,
            transport,
            telemetry,
            nexo: NexoGateway::spawn_with_config(nexo_config, reconnect_interval),
            chat_event_tx,
            chat_event_rx,
            autonomy: AutonomyRunner::servo_scan(),
            active_controllers: BTreeMap::new(),
            desired_mode,
            desired_servo,
            active_control_state: ControlState::default(),
            health_check_interval: Duration::from_millis(session.health_check_interval_ms.max(1)),
            reconnect_interval,
            active_desired_state_interval: ACTIVE_DESIRED_STATE_INTERVAL,
            input_poll_interval: INPUT_POLL_INTERVAL,
            last_contact_at: now,
            last_desired_state_at: None,
            applied_link_timeout_ms: None,
        }
    }

    /// Run the robot loop until the selected input source requests shutdown.
    pub async fn run<I, O>(&mut self, input: &mut I, output: &mut O) -> Result<()>
    where
        I: CommandInputSource,
        O: SessionOutput,
    {
        if self.transport.is_connected() {
            output.set_connection_status("connected".to_string())?;
        }
        output.set_desired_mode(self.desired_mode)?;
        output.set_control_state(self.active_control_state)?;

        'run: loop {
            self.drain_chat_events(output)?;

            if !self.transport.is_connected() {
                match self.transport.try_connect().await {
                    Ok(()) => {
                        input.resume()?;
                        self.restore_desired_state_after_connect(Instant::now());
                        output.set_connection_status("connected".to_string())?;
                        output.set_desired_mode(self.desired_mode)?;
                        output.set_control_state(self.active_control_state)?;
                        self.query_controller_status(output).await?;
                        self.exchange_desired_state(output).await?;
                        output.log(LogLevel::Info, "pico transport connected".to_string())?;
                    }
                    Err(error) => {
                        input.suspend()?;
                        self.active_control_state = ControlState::default();
                        self.last_desired_state_at = None;
                        self.applied_link_timeout_ms = None;
                        output.set_connection_status(format!(
                            "disconnected; retrying in {} ms",
                            self.reconnect_interval.as_millis()
                        ))?;
                        output.set_control_state(self.active_control_state)?;
                        output.log(
                            LogLevel::Warn,
                            format!(
                                "pico transport unavailable; pausing operator input until reconnect: {error}"
                            ),
                        )?;

                        let reconnect_deadline = Instant::now() + self.reconnect_interval;
                        while Instant::now() < reconnect_deadline {
                            self.drain_chat_events(output)?;

                            let wait = reconnect_deadline
                                .saturating_duration_since(Instant::now())
                                .min(self.input_poll_interval);
                            if wait.is_zero() {
                                break;
                            }

                            let events = self.collect_input_events(input, wait)?;
                            if !events.is_empty() {
                                match self.handle_input_events(input, output, events).await {
                                    Ok(true) => break 'run,
                                    Ok(false) => {}
                                    Err(error) => {
                                        output.log(
                                            LogLevel::Warn,
                                            format!(
                                                "command unavailable while transport is disconnected: {error}"
                                            ),
                                        )?;
                                    }
                                }
                            }
                        }
                        continue;
                    }
                }
            }

            let events = self.collect_input_events(input, self.input_poll_interval)?;
            if !events.is_empty() {
                match self.handle_input_events(input, output, events).await {
                    Ok(true) => break,
                    Ok(false) => continue,
                    Err(error) => {
                        self.handle_transport_failure(
                            input,
                            output,
                            format!(
                                "pico transport command failed; pausing operator input until reconnect: {error}"
                            ),
                        )?;
                        continue;
                    }
                }
            }

            if self.desired_mode == Mode::Autonomous {
                let target = self.autonomy.target_at(Instant::now());
                if self.apply_autonomous_target(target, output).await? {
                    continue;
                }
            }

            let should_refresh = self
                .last_desired_state_at
                .map(|last| last.elapsed() >= self.desired_state_sync_interval())
                .unwrap_or(true);
            if should_refresh && let Err(error) = self.exchange_desired_state(output).await {
                self.handle_transport_failure(
                    input,
                    output,
                    format!(
                        "pico transport desired-state sync failed; pausing operator input until reconnect: {error}"
                    ),
                )?;
                continue;
            }
        }

        Ok(())
    }

    fn drain_chat_events<O>(&mut self, output: &mut O) -> Result<()>
    where
        O: SessionOutput,
    {
        while let Ok((level, message)) = self.chat_event_rx.try_recv() {
            output.log(level, message)?;
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn active_controllers(&self) -> impl Iterator<Item = &ControllerInfo> {
        self.active_controllers.values()
    }

    /// Handle one operator command.
    pub async fn step(&mut self, command: BrainCommand) -> Result<BrainStep> {
        match command {
            BrainCommand::Quit => Ok(BrainStep::Exit),
            BrainCommand::Stop => {
                self.desired_mode = self.router.default_mode;
                self.active_control_state = ControlState::default();
                self.desired_servo = RouterPolicy::centered_servo();
                self.set_link_timeout_raw(self.desired_state_link_timeout_ms())
                    .await?;
                Ok(BrainStep::Continue(
                    self.exchange_desired_state_raw().await?,
                ))
            }
            BrainCommand::SetMode(mode) => {
                let previous_mode = self.desired_mode;
                self.desired_mode = mode;
                if mode == Mode::Autonomous {
                    self.autonomy.reset();
                    let target = self.autonomy.target_at(Instant::now());
                    self.active_control_state = ControlState {
                        drive: target.drive,
                    };
                    self.desired_servo = target.servo;
                } else if mode == Mode::Fault {
                    self.active_control_state = ControlState::default();
                    self.desired_servo = RouterPolicy::centered_servo();
                } else if previous_mode == Mode::Autonomous {
                    self.active_control_state = ControlState::default();
                }
                self.set_link_timeout_raw(self.desired_state_link_timeout_ms())
                    .await?;
                Ok(BrainStep::Continue(
                    self.exchange_desired_state_raw().await?,
                ))
            }
            BrainCommand::Chat(_) => Err(anyhow!("chat commands require session output")),
            BrainCommand::Servo { pan, tilt } => {
                self.desired_servo = ServoCommand {
                    pan: ServoTicks(pan),
                    tilt: ServoTicks(tilt),
                };
                self.set_link_timeout_raw(self.desired_state_link_timeout_ms())
                    .await?;
                Ok(BrainStep::Continue(
                    self.exchange_desired_state_raw().await?,
                ))
            }
        }
    }

    async fn query_controller_status<O>(&mut self, output: &mut O) -> Result<()>
    where
        O: SessionOutput,
    {
        let controllers = self.transport.connected_controllers();

        if controllers.is_empty() {
            return Err(anyhow!("controller discovery found no active controllers"));
        }

        for controller in controllers {
            self.last_contact_at = Instant::now();
            self.handle_telemetry(Telemetry::Status(controller.status), output)?;
        }

        Ok(())
    }

    fn desired_state_command(&self) -> Command {
        self.router.desired_state_command(
            self.desired_mode,
            self.active_control_state.drive,
            self.desired_servo,
        )
    }

    fn is_default_desired_state(&self) -> bool {
        self.desired_mode == self.router.default_mode
            && self.active_control_state.drive.is_none()
            && self.desired_servo == RouterPolicy::centered_servo()
    }

    fn desired_state_sync_interval(&self) -> Duration {
        if self.is_default_desired_state() {
            self.health_check_interval
        } else {
            self.active_desired_state_interval
        }
    }

    fn desired_state_link_timeout_ms(&self) -> u32 {
        let interval_ms = self.desired_state_sync_interval().as_millis();
        let min_timeout_ms = MIN_LINK_TIMEOUT.as_millis();
        let timeout_ms = interval_ms.saturating_mul(2).max(min_timeout_ms);
        timeout_ms.min(u128::from(u32::MAX)) as u32
    }

    async fn set_link_timeout_raw(&mut self, milliseconds: u32) -> Result<Option<Telemetry>> {
        if self.applied_link_timeout_ms == Some(milliseconds) {
            return Ok(None);
        }

        let response = self
            .transport
            .exchange_command(RouterPolicy::link_timeout_update(milliseconds))
            .await?;
        self.applied_link_timeout_ms = Some(milliseconds);
        self.last_contact_at = Instant::now();
        Ok(response)
    }

    async fn ensure_link_timeout_configured<O>(&mut self, output: &mut O) -> Result<()>
    where
        O: SessionOutput,
    {
        if let Some(telemetry) = self
            .set_link_timeout_raw(self.desired_state_link_timeout_ms())
            .await?
        {
            self.handle_telemetry(telemetry, output)?;
        }

        Ok(())
    }

    fn restore_desired_state_after_connect(&mut self, now: Instant) {
        self.autonomy.reset();
        self.active_control_state = ControlState::default();
        self.last_desired_state_at = None;
        self.applied_link_timeout_ms = None;
        self.last_contact_at = now;

        match self.desired_mode {
            Mode::Autonomous => {
                let target = self.autonomy.target_at(now);
                self.active_control_state = ControlState {
                    drive: target.drive,
                };
                self.desired_servo = target.servo;
            }
            Mode::Teleop => {}
            Mode::Fault => {
                self.desired_servo = RouterPolicy::centered_servo();
            }
        }
    }

    async fn exchange_desired_state_raw(&mut self) -> Result<Option<Telemetry>> {
        let response = self
            .transport
            .exchange_command(self.desired_state_command())
            .await?;

        let now = Instant::now();
        self.last_contact_at = now;
        self.last_desired_state_at = Some(now);

        Ok(response)
    }

    async fn exchange_desired_state<O>(&mut self, output: &mut O) -> Result<()>
    where
        O: SessionOutput,
    {
        self.ensure_link_timeout_configured(output).await?;

        if let Some(telemetry) = self.exchange_desired_state_raw().await? {
            self.handle_telemetry(telemetry, output)?;
        }

        Ok(())
    }

    fn refresh_active_control_hold<I>(&mut self, input: &mut I) -> Result<()>
    where
        I: CommandInputSource,
    {
        if self.active_control_state.drive.is_some() {
            input.extend_active_control(COMMAND_COMPLETION_HOLD_GRACE)?;
        }

        Ok(())
    }

    async fn apply_desired_mode<O>(&mut self, mode: Mode, output: &mut O) -> Result<()>
    where
        O: SessionOutput,
    {
        let previous_mode = self.desired_mode;
        self.desired_mode = mode;
        output.set_desired_mode(self.desired_mode)?;

        if mode == Mode::Autonomous {
            self.autonomy.reset();
            output.log(
                LogLevel::Info,
                format!("autonomy plan active: {}", self.autonomy.plan_name()),
            )?;
            let target = self.autonomy.target_at(Instant::now());
            self.active_control_state = ControlState {
                drive: target.drive,
            };
            self.desired_servo = target.servo;
            self.last_desired_state_at = None;
            output.set_control_state(self.active_control_state)?;
            if !self.transport.is_connected() {
                return Ok(());
            }
            return self.exchange_desired_state(output).await;
        }

        if mode == Mode::Fault {
            self.active_control_state = ControlState::default();
            self.desired_servo = RouterPolicy::centered_servo();
            output.set_control_state(self.active_control_state)?;
        } else if previous_mode == Mode::Autonomous && self.active_control_state.drive.is_some() {
            self.active_control_state = ControlState::default();
            output.set_control_state(self.active_control_state)?;
        }

        self.last_desired_state_at = None;

        if !self.transport.is_connected() {
            return Ok(());
        }

        self.exchange_desired_state(output).await
    }

    async fn apply_desired_servo<O>(&mut self, pan: u16, tilt: u16, output: &mut O) -> Result<()>
    where
        O: SessionOutput,
    {
        self.desired_servo = ServoCommand {
            pan: ServoTicks(pan),
            tilt: ServoTicks(tilt),
        };

        self.last_desired_state_at = None;

        if !self.transport.is_connected() {
            return Ok(());
        }

        self.exchange_desired_state(output).await
    }

    async fn stop_desired_motion<O>(&mut self, output: &mut O) -> Result<()>
    where
        O: SessionOutput,
    {
        self.desired_mode = self.router.default_mode;
        self.active_control_state = ControlState::default();
        self.desired_servo = RouterPolicy::centered_servo();
        self.last_desired_state_at = None;
        output.set_desired_mode(self.desired_mode)?;
        output.set_control_state(self.active_control_state)?;

        if !self.transport.is_connected() {
            return Ok(());
        }

        self.exchange_desired_state(output).await
    }

    async fn apply_autonomous_target<O>(
        &mut self,
        target: AutonomousTarget,
        output: &mut O,
    ) -> Result<bool>
    where
        O: SessionOutput,
    {
        let control_state = ControlState {
            drive: target.drive,
        };
        if self.active_control_state == control_state && self.desired_servo == target.servo {
            return Ok(false);
        }

        self.active_control_state = control_state;
        self.desired_servo = target.servo;
        self.last_desired_state_at = None;
        output.set_control_state(self.active_control_state)?;
        self.exchange_desired_state(output).await?;
        Ok(true)
    }

    async fn apply_control_state<O>(
        &mut self,
        control_state: ControlState,
        output: &mut O,
    ) -> Result<()>
    where
        O: SessionOutput,
    {
        if self.active_control_state == control_state {
            return Ok(());
        }

        self.active_control_state = control_state;
        self.last_desired_state_at = None;
        output.set_control_state(control_state)?;

        if !self.transport.is_connected() {
            return Ok(());
        }

        self.exchange_desired_state(output).await
    }

    fn collect_input_events<I>(
        &mut self,
        input: &mut I,
        timeout: Duration,
    ) -> Result<Vec<InputEvent>>
    where
        I: CommandInputSource,
    {
        let Some(first_event) = input.poll_event(timeout)? else {
            return Ok(Vec::new());
        };

        let mut events = vec![first_event];
        while let Some(event) = input.poll_event(Duration::ZERO)? {
            events.push(event);
        }

        Ok(events)
    }

    async fn handle_input_events<I, O>(
        &mut self,
        input: &mut I,
        output: &mut O,
        events: Vec<InputEvent>,
    ) -> Result<bool>
    where
        I: CommandInputSource,
        O: SessionOutput,
    {
        let mut pending_control = None;

        for event in events {
            match event {
                InputEvent::ControllerConnected(controller) => {
                    self.active_controllers
                        .insert(controller.id.clone(), controller.clone());
                    output.log(
                        LogLevel::Info,
                        format!(
                            "controller connected: {} ({})",
                            controller.id, controller.display_name
                        ),
                    )?;
                }
                InputEvent::ControllerDisconnected(controller) => {
                    self.active_controllers.remove(&controller.id);
                    output.log(
                        LogLevel::Info,
                        format!(
                            "controller disconnected: {} ({})",
                            controller.id, controller.display_name
                        ),
                    )?;
                }
                InputEvent::Control(control_state) => {
                    if self.desired_mode == Mode::Teleop {
                        pending_control = Some(control_state);
                    }
                }
                InputEvent::Warning(warning) => {
                    output.log(LogLevel::Info, warning.to_string())?;
                }
                InputEvent::Command(command) => {
                    if let Some(control_state) = pending_control.take() {
                        self.apply_control_state(control_state, output).await?;
                    }

                    match command {
                        BrainCommand::Quit => return Ok(true),
                        BrainCommand::Stop => {
                            self.stop_desired_motion(output).await?;
                            self.refresh_active_control_hold(input)?;
                        }
                        BrainCommand::SetMode(mode) => {
                            self.apply_desired_mode(mode, output).await?;
                            self.refresh_active_control_hold(input)?;
                        }
                        BrainCommand::Chat(prompt) => {
                            self.handle_chat_command(prompt, output).await?;
                        }
                        BrainCommand::Servo { pan, tilt } => {
                            self.apply_desired_servo(pan, tilt, output).await?;
                            self.refresh_active_control_hold(input)?;
                        }
                    }
                }
            }
        }

        if let Some(control_state) = pending_control {
            self.apply_control_state(control_state, output).await?;
        }

        Ok(false)
    }

    async fn handle_chat_command<O>(&mut self, prompt: String, output: &mut O) -> Result<()>
    where
        O: SessionOutput,
    {
        output.log(LogLevel::Info, "sending chat prompt".to_string())?;

        let gateway = self.nexo.clone();
        let chat_event_tx = self.chat_event_tx.clone();
        tokio::spawn(async move {
            let event = match gateway.chat(prompt).await {
                Ok(reply) => (LogLevel::Info, format!("nexo reply: {}", reply.content)),
                Err(error) => (LogLevel::Error, format!("nexo chat failed: {error:#}")),
            };
            let _ = chat_event_tx.send(event);
        });

        Ok(())
    }

    fn handle_transport_failure<I, O>(
        &mut self,
        input: &mut I,
        output: &mut O,
        message: String,
    ) -> Result<()>
    where
        I: CommandInputSource,
        O: SessionOutput,
    {
        input.suspend()?;
        self.autonomy.reset();
        self.active_controllers.clear();
        self.active_control_state = ControlState::default();
        self.last_desired_state_at = None;
        self.applied_link_timeout_ms = None;
        output.set_connection_status(format!(
            "disconnected; retrying in {} ms",
            self.reconnect_interval.as_millis()
        ))?;
        output.set_desired_mode(self.desired_mode)?;
        output.set_control_state(self.active_control_state)?;
        output.set_distance(None)?;
        output.log(LogLevel::Warn, message)?;
        self.transport.disconnect();
        Ok(())
    }

    fn handle_telemetry<O>(&mut self, telemetry: Telemetry, output: &mut O) -> Result<()>
    where
        O: SessionOutput,
    {
        if let Some(distance) = latest_range_sample(&telemetry) {
            output.set_distance(Some(distance))?;
        }
        output.log(
            LogLevel::Info,
            format!("telemetry {}: {telemetry:?}", telemetry.kind()),
        )?;
        self.telemetry.publish(&telemetry);
        Ok(())
    }
}

fn latest_range_sample(telemetry: &Telemetry) -> Option<RangeTelemetry> {
    match telemetry {
        Telemetry::Status(status) => status.range,
        Telemetry::DesiredState(desired_state) => desired_state.range,
        Telemetry::Range(range) => Some(*range),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::{Result, anyhow};
    use mortimmy_core::{Mode, ServoTicks};
    use mortimmy_protocol::messages::{commands::ServoCommand, telemetry::{RangeTelemetry, Telemetry}};
    use tokio::time::Instant;

    use super::COMMAND_COMPLETION_HOLD_GRACE;

    use crate::{
        brain::{
            BrainCommand, BrainStep, RobotBrain,
            command_mapping::RouterPolicy,
            transport::{BrainTransport, TransportBackendKind},
        },
        config::{LogLevel, SessionConfig},
        input::{CommandInputSource, ControlState, DriveIntent, InputEvent, ScriptedInput},
        serial::SerialConfig,
        telemetry::{TelemetryConfig, TelemetryFanout},
        tui::{NullSessionOutput, SessionOutput},
    };

    #[derive(Default)]
    struct TrackingInput {
        refreshed: Vec<Duration>,
        polled: Vec<Duration>,
        pending_events: std::collections::VecDeque<InputEvent>,
    }

    #[derive(Default)]
    struct RecordingOutput {
        logs: Vec<String>,
        connection_status: String,
        control_state: ControlState,
        desired_mode: Mode,
        distance: Option<RangeTelemetry>,
    }

    impl SessionOutput for RecordingOutput {
        fn log(&mut self, _level: LogLevel, message: String) -> Result<()> {
            self.logs.push(message);
            Ok(())
        }

        fn set_connection_status(&mut self, status: String) -> Result<()> {
            self.connection_status = status;
            Ok(())
        }

        fn set_control_state(&mut self, control_state: ControlState) -> Result<()> {
            self.control_state = control_state;
            Ok(())
        }

        fn set_desired_mode(&mut self, mode: Mode) -> Result<()> {
            self.desired_mode = mode;
            Ok(())
        }

        fn set_distance(&mut self, distance: Option<RangeTelemetry>) -> Result<()> {
            self.distance = distance;
            Ok(())
        }
    }

    impl CommandInputSource for TrackingInput {
        fn next_event(&mut self) -> Result<InputEvent> {
            Err(anyhow!("tracking input is eventless"))
        }

        fn poll_event(&mut self, timeout: Duration) -> Result<Option<InputEvent>> {
            self.polled.push(timeout);
            Ok(self.pending_events.pop_front())
        }

        fn extend_active_control(&mut self, duration: Duration) -> Result<()> {
            self.refreshed.push(duration);
            Ok(())
        }
    }

    #[tokio::test]
    async fn drive_command_returns_motor_state_telemetry() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(
                TransportBackendKind::Loopback,
                SerialConfig::default(),
                Duration::from_secs(2),
            )
            .unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );

        match brain
            .step(BrainCommand::SetMode(Mode::Teleop))
            .await
            .unwrap()
        {
            BrainStep::Continue(Some(Telemetry::DesiredState(telemetry))) => {
                assert_eq!(telemetry.mode, Mode::Teleop);
                assert_eq!(telemetry.drive.left_pwm.0, 0);
                assert_eq!(telemetry.drive.right_pwm.0, 0);
            }
            other => panic!("unexpected brain step: {other:?}"),
        }
    }

    #[tokio::test]
    async fn stop_command_returns_default_desired_state_telemetry() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(
                TransportBackendKind::Loopback,
                SerialConfig::default(),
                Duration::from_secs(2),
            )
            .unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );
        brain.desired_mode = Mode::Teleop;
        brain.active_control_state = ControlState {
            drive: Some(DriveIntent {
                forward: DriveIntent::AXIS_MAX,
                turn: 0,
                speed: 300,
            }),
        };

        match brain.step(BrainCommand::Stop).await.unwrap() {
            BrainStep::Continue(Some(Telemetry::DesiredState(telemetry))) => {
                assert_eq!(telemetry.mode, Mode::Teleop);
                assert_eq!(telemetry.drive.left_pwm.0, 0);
                assert_eq!(telemetry.drive.right_pwm.0, 0);
            }
            other => panic!("unexpected brain step: {other:?}"),
        }
    }

    #[tokio::test]
    async fn handle_telemetry_updates_distance_from_latest_range_sample() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(
                TransportBackendKind::Loopback,
                SerialConfig::default(),
                Duration::from_secs(2),
            )
            .unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );
        let mut output = RecordingOutput::default();

        brain
            .handle_telemetry(
                Telemetry::Range(RangeTelemetry {
                    distance_mm: mortimmy_core::Millimeters(412),
                    quality: 100,
                }),
                &mut output,
            )
            .unwrap();

        assert_eq!(
            output.distance,
            Some(RangeTelemetry {
                distance_mm: mortimmy_core::Millimeters(412),
                quality: 100,
            })
        );
    }

    #[tokio::test]
    async fn transport_failure_preserves_requested_teleop_mode_for_reconnect() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(
                TransportBackendKind::Loopback,
                SerialConfig::default(),
                Duration::from_secs(2),
            )
            .unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );
        brain.desired_mode = Mode::Teleop;
        brain.desired_servo = ServoCommand {
            pan: ServoTicks(12),
            tilt: ServoTicks(18),
        };
        brain.active_control_state = ControlState {
            drive: Some(DriveIntent {
                forward: DriveIntent::AXIS_MAX,
                turn: 0,
                speed: 300,
            }),
        };

        let mut input = TrackingInput::default();
        let mut output = NullSessionOutput;

        brain
            .handle_transport_failure(&mut input, &mut output, "link lost".to_string())
            .unwrap();

        assert_eq!(brain.desired_mode, Mode::Teleop);
        assert_eq!(brain.active_control_state, ControlState::default());
        assert_eq!(
            brain.desired_servo,
            ServoCommand {
                pan: ServoTicks(12),
                tilt: ServoTicks(18),
            }
        );
    }

    #[tokio::test]
    async fn reconnect_restores_autonomous_target_after_disconnect() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(
                TransportBackendKind::Loopback,
                SerialConfig::default(),
                Duration::from_secs(2),
            )
            .unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );
        brain.desired_mode = Mode::Autonomous;
        brain.active_control_state = ControlState {
            drive: Some(DriveIntent {
                forward: DriveIntent::AXIS_MAX,
                turn: 0,
                speed: 300,
            }),
        };

        let mut input = TrackingInput::default();
        let mut output = NullSessionOutput;
        brain
            .handle_transport_failure(&mut input, &mut output, "link lost".to_string())
            .unwrap();

        let now = Instant::now();
        let expected_target = brain.autonomy.target_at(now);
        brain.restore_desired_state_after_connect(now);

        assert_eq!(brain.desired_mode, Mode::Autonomous);
        assert_eq!(
            brain.active_control_state,
            ControlState {
                drive: expected_target.drive,
            }
        );
        assert_eq!(brain.desired_servo, expected_target.servo);
    }

    #[tokio::test]
    async fn run_loop_consumes_scripted_commands_until_quit() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(
                TransportBackendKind::Loopback,
                SerialConfig::default(),
                Duration::from_secs(2),
            )
            .unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );
        let mut input = ScriptedInput::new([
            InputEvent::Command(BrainCommand::Stop),
            InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 100,
                }),
            }),
            InputEvent::Control(ControlState { drive: None }),
            InputEvent::Command(BrainCommand::Quit),
        ]);
        let mut output = NullSessionOutput;

        brain.run(&mut input, &mut output).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires a local nexo-gateway on ws://127.0.0.1:6969"]
    async fn run_loop_logs_live_nexo_chat_reply() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(
                TransportBackendKind::Loopback,
                SerialConfig::default(),
                Duration::from_secs(2),
            )
            .unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );
        let mut input = ScriptedInput::new([
            InputEvent::Command(BrainCommand::Chat(
                "Reply with a short greeting for mortimmy.".to_string(),
            )),
            InputEvent::Command(BrainCommand::Quit),
        ]);
        let mut output = RecordingOutput::default();

        tokio::time::timeout(Duration::from_secs(90), brain.run(&mut input, &mut output))
            .await
            .expect("brain run timed out")
            .unwrap();

        assert!(output.logs.iter().any(|message| {
            message.starts_with("nexo reply: ")
                && !message.trim_start_matches("nexo reply: ").trim().is_empty()
        }));
    }

    #[tokio::test]
    async fn run_loop_accepts_combined_drive_and_discrete_command_bursts() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(
                TransportBackendKind::Loopback,
                SerialConfig::default(),
                Duration::from_secs(2),
            )
            .unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );
        let mut input = ScriptedInput::new([
            InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            }),
            InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: -DriveIntent::AXIS_MAX,
                    speed: 300,
                }),
            }),
            InputEvent::Command(BrainCommand::SetMode(Mode::Teleop)),
            InputEvent::Command(BrainCommand::Quit),
        ]);
        let mut output = NullSessionOutput;

        brain.run(&mut input, &mut output).await.unwrap();
        assert_eq!(
            brain.active_control_state,
            ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: -DriveIntent::AXIS_MAX,
                    speed: 300,
                }),
            }
        );
    }

    #[tokio::test]
    async fn discrete_command_while_drive_is_active_refreshes_input_hold() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(
                TransportBackendKind::Loopback,
                SerialConfig::default(),
                Duration::from_secs(2),
            )
            .unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );
        brain.active_control_state = ControlState {
            drive: Some(DriveIntent {
                forward: DriveIntent::AXIS_MAX,
                turn: 0,
                speed: 300,
            }),
        };

        let mut input = TrackingInput::default();
        let mut output = NullSessionOutput;

        let should_exit = brain
            .handle_input_events(
                &mut input,
                &mut output,
                vec![InputEvent::Command(BrainCommand::Stop)],
            )
            .await
            .unwrap();

        assert!(!should_exit);
        assert_eq!(input.refreshed, vec![COMMAND_COMPLETION_HOLD_GRACE]);
    }

    #[tokio::test]
    async fn chat_command_returns_without_waiting_for_gateway_reply() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(
                TransportBackendKind::Loopback,
                SerialConfig::default(),
                Duration::from_secs(2),
            )
            .unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );
        let mut input = TrackingInput::default();
        let mut output = RecordingOutput::default();

        let should_exit = tokio::time::timeout(
            Duration::from_millis(50),
            brain.handle_input_events(
                &mut input,
                &mut output,
                vec![InputEvent::Command(BrainCommand::Chat(
                    "reply whenever you can".to_string(),
                ))],
            ),
        )
        .await
        .expect("chat command handling timed out")
        .unwrap();

        assert!(!should_exit);
        assert!(
            output
                .logs
                .iter()
                .any(|message| message == "sending chat prompt")
        );
    }

    #[tokio::test]
    async fn disconnected_mode_change_updates_output_without_transport_roundtrip() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(
                TransportBackendKind::Loopback,
                SerialConfig::default(),
                Duration::from_secs(2),
            )
            .unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );
        let mut input = TrackingInput::default();
        let mut output = RecordingOutput::default();

        let should_exit = brain
            .handle_input_events(
                &mut input,
                &mut output,
                vec![InputEvent::Command(BrainCommand::SetMode(Mode::Fault))],
            )
            .await
            .unwrap();

        assert!(!should_exit);
        assert_eq!(brain.desired_mode, Mode::Fault);
        assert_eq!(output.desired_mode, Mode::Fault);
        assert_eq!(output.control_state, ControlState::default());
    }

    #[tokio::test]
    async fn collect_input_events_uses_requested_timeout_and_drains_followups() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(
                TransportBackendKind::Loopback,
                SerialConfig::default(),
                Duration::from_secs(2),
            )
            .unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );
        let mut input = TrackingInput {
            pending_events: std::collections::VecDeque::from([
                InputEvent::Command(BrainCommand::Quit),
                InputEvent::Command(BrainCommand::Stop),
            ]),
            ..TrackingInput::default()
        };

        let events = brain
            .collect_input_events(&mut input, Duration::from_millis(1234))
            .unwrap();

        assert_eq!(
            input.polled,
            vec![Duration::from_millis(1234), Duration::ZERO, Duration::ZERO]
        );
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], InputEvent::Command(BrainCommand::Quit));
        assert_eq!(events[1], InputEvent::Command(BrainCommand::Stop));
    }

    #[tokio::test]
    async fn controller_lifecycle_events_update_active_registry() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(
                TransportBackendKind::Loopback,
                SerialConfig::default(),
                Duration::from_secs(2),
            )
            .unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );
        let controller = crate::input::ControllerInfo::new(
            crate::input::ControllerId::new(crate::input::ControllerKind::Keyboard, "local"),
            "Local Keyboard",
        );
        let mut input = TrackingInput::default();
        let mut output = NullSessionOutput;

        let should_exit = brain
            .handle_input_events(
                &mut input,
                &mut output,
                vec![InputEvent::ControllerConnected(controller.clone())],
            )
            .await
            .unwrap();

        assert!(!should_exit);
        assert_eq!(brain.active_controllers().count(), 1);
        assert_eq!(
            brain.active_controllers.get(&controller.id),
            Some(&controller)
        );

        let should_exit = brain
            .handle_input_events(
                &mut input,
                &mut output,
                vec![InputEvent::ControllerDisconnected(controller.clone())],
            )
            .await
            .unwrap();

        assert!(!should_exit);
        assert!(brain.active_controllers().next().is_none());
    }
}
