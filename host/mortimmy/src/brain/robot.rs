use anyhow::{Result, anyhow};
use mortimmy_core::{Mode, ServoTicks};
use mortimmy_protocol::messages::{
    command::Command,
    commands::ServoCommand,
    telemetry::Telemetry,
};
use tokio::time::{Duration, Instant, sleep};

use crate::{
    brain::autonomy::{AutonomyRunner, AutonomousTarget},
    config::{LogLevel, SessionConfig},
    input::{CommandInputSource, ControlState, InputEvent},
    routing::RouterPolicy,
    telemetry::TelemetryFanout,
    ui::SessionOutput,
};

use super::{BrainCommand, transport::BrainTransport};

const ACTIVE_DESIRED_STATE_INTERVAL: Duration = Duration::from_millis(75);
const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(10);
const COMMAND_COMPLETION_HOLD_GRACE: Duration = Duration::from_millis(750);
const MIN_LINK_TIMEOUT: Duration = Duration::from_millis(250);

/// Result of handling a single operator command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    autonomy: AutonomyRunner,
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
    pub fn new(
        router: RouterPolicy,
        transport: BrainTransport,
        telemetry: TelemetryFanout,
        session: SessionConfig,
    ) -> Self {
        let now = Instant::now();
        let desired_mode = router.default_mode;
        let desired_servo = router.centered_servo();
        Self {
            router,
            transport,
            telemetry,
            autonomy: AutonomyRunner::servo_scan(),
            desired_mode,
            desired_servo,
            active_control_state: ControlState::default(),
            health_check_interval: Duration::from_millis(session.health_check_interval_ms.max(1)),
            reconnect_interval: Duration::from_millis(session.reconnect_interval_ms.max(1)),
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
        output.set_control_state(self.active_control_state)?;

        loop {
            if !self.transport.is_connected() {
                match self.transport.try_connect().await {
                    Ok(()) => {
                        input.resume()?;
                        self.restore_desired_state_after_connect(Instant::now());
                        output.set_connection_status("connected".to_string())?;
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
                        sleep(self.reconnect_interval).await;
                        continue;
                    }
                }
            }

            let events = self.collect_input_events(input)?;
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
            if should_refresh {
                if let Err(error) = self.exchange_desired_state(output).await {
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
        }

        Ok(())
    }

    /// Handle one operator command.
    pub async fn step(&mut self, command: BrainCommand) -> Result<BrainStep> {
        match command {
            BrainCommand::Quit => Ok(BrainStep::Exit),
            BrainCommand::Ping => self.exchange(self.router.ping_command()).await,
            BrainCommand::Stop => {
                self.desired_mode = self.router.default_mode;
                self.active_control_state = ControlState::default();
                self.desired_servo = self.router.centered_servo();
                self.set_link_timeout_raw(self.desired_state_link_timeout_ms()).await?;
                Ok(BrainStep::Continue(self.exchange_desired_state_raw().await?))
            }
            BrainCommand::SetMode(mode) => {
                let previous_mode = self.desired_mode;
                self.desired_mode = mode;
                if mode == Mode::Autonomous {
                    self.autonomy.reset();
                    let target = self.autonomy.target_at(Instant::now());
                    self.active_control_state = ControlState { drive: target.drive };
                    self.desired_servo = target.servo;
                } else if mode == Mode::Fault {
                    self.active_control_state = ControlState::default();
                    self.desired_servo = self.router.centered_servo();
                } else if previous_mode == Mode::Autonomous {
                    self.active_control_state = ControlState::default();
                }
                self.set_link_timeout_raw(self.desired_state_link_timeout_ms()).await?;
                Ok(BrainStep::Continue(self.exchange_desired_state_raw().await?))
            }
            BrainCommand::Servo { pan, tilt } => {
                self.desired_servo = ServoCommand {
                    pan: ServoTicks(pan),
                    tilt: ServoTicks(tilt),
                };
                self.set_link_timeout_raw(self.desired_state_link_timeout_ms()).await?;
                Ok(BrainStep::Continue(self.exchange_desired_state_raw().await?))
            }
        }
    }

    async fn exchange(&mut self, command: Command) -> Result<BrainStep> {
        let response = self.transport.exchange_command(command).await?;
        Ok(BrainStep::Continue(response))
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
        self.router
            .desired_state_command(self.desired_mode, self.active_control_state.drive, self.desired_servo)
    }

    fn is_default_desired_state(&self) -> bool {
        self.desired_mode == self.router.default_mode
            && self.active_control_state.drive.is_none()
            && self.desired_servo == self.router.centered_servo()
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

        let response = self.transport.exchange_command(self.router.link_timeout_update(milliseconds)).await?;
        self.applied_link_timeout_ms = Some(milliseconds);
        self.last_contact_at = Instant::now();
        Ok(response)
    }

    async fn ensure_link_timeout_configured<O>(&mut self, output: &mut O) -> Result<()>
    where
        O: SessionOutput,
    {
        if let Some(telemetry) = self.set_link_timeout_raw(self.desired_state_link_timeout_ms()).await? {
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
                self.active_control_state = ControlState { drive: target.drive };
                self.desired_servo = target.servo;
            }
            Mode::Teleop => {}
            Mode::Fault => {
                self.desired_servo = self.router.centered_servo();
            }
        }
    }

    async fn exchange_desired_state_raw(&mut self) -> Result<Option<Telemetry>> {
        let response = self.transport.exchange_command(self.desired_state_command()).await?;

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

        if mode == Mode::Autonomous {
            self.autonomy.reset();
            output.log(
                LogLevel::Info,
                format!("autonomy plan active: {}", self.autonomy.plan_name()),
            )?;
            let target = self.autonomy.target_at(Instant::now());
            self.active_control_state = ControlState { drive: target.drive };
            self.desired_servo = target.servo;
            self.last_desired_state_at = None;
            output.set_control_state(self.active_control_state)?;
            return self.exchange_desired_state(output).await;
        }

        if mode == Mode::Fault {
            self.active_control_state = ControlState::default();
            self.desired_servo = self.router.centered_servo();
            output.set_control_state(self.active_control_state)?;
        } else if previous_mode == Mode::Autonomous && self.active_control_state.drive.is_some() {
            self.active_control_state = ControlState::default();
            output.set_control_state(self.active_control_state)?;
        }

        self.last_desired_state_at = None;

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

        self.exchange_desired_state(output).await
    }

    async fn stop_desired_motion<O>(&mut self, output: &mut O) -> Result<()>
    where
        O: SessionOutput,
    {
        self.desired_mode = self.router.default_mode;
        self.active_control_state = ControlState::default();
        self.desired_servo = self.router.centered_servo();
        self.last_desired_state_at = None;
        output.set_control_state(self.active_control_state)?;
        self.exchange_desired_state(output).await
    }

    async fn apply_autonomous_target<O>(&mut self, target: AutonomousTarget, output: &mut O) -> Result<bool>
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

    async fn apply_control_state<O>(&mut self, control_state: ControlState, output: &mut O) -> Result<()>
    where
        O: SessionOutput,
    {
        if self.active_control_state == control_state {
            return Ok(());
        }

        self.active_control_state = control_state;
        self.last_desired_state_at = None;
        output.set_control_state(control_state)?;
        self.exchange_desired_state(output).await
    }

    fn collect_input_events<I>(&mut self, input: &mut I) -> Result<Vec<InputEvent>>
    where
        I: CommandInputSource,
    {
        let Some(first_event) = input.poll_event(self.input_poll_interval)? else {
            return Ok(Vec::new());
        };

        let mut events = vec![first_event];
        while let Some(event) = input.poll_event(Duration::ZERO)? {
            events.push(event);
        }

        Ok(events)
    }

    async fn handle_input_events<I, O>(&mut self, input: &mut I, output: &mut O, events: Vec<InputEvent>) -> Result<bool>
    where
        I: CommandInputSource,
        O: SessionOutput,
    {
        let mut pending_control = None;

        for event in events {
            match event {
                InputEvent::Control(control_state) => {
                    if self.desired_mode == Mode::Teleop {
                        pending_control = Some(control_state);
                    }
                }
                InputEvent::Warning(warning) => {
                    output.log(LogLevel::Warn, warning.to_string())?;
                }
                InputEvent::Command(command) => {
                    if let Some(control_state) = pending_control.take() {
                        self.apply_control_state(control_state, output).await?;
                    }

                    match command {
                        BrainCommand::Quit => return Ok(true),
                        BrainCommand::Ping => match self.step(BrainCommand::Ping).await? {
                            BrainStep::Continue(Some(telemetry)) => {
                                self.last_contact_at = Instant::now();
                                self.refresh_active_control_hold(input)?;
                                self.handle_telemetry(telemetry, output)?;
                            }
                            BrainStep::Continue(None) => {
                                self.last_contact_at = Instant::now();
                                self.refresh_active_control_hold(input)?;
                            }
                            BrainStep::Exit => return Ok(true),
                        },
                        BrainCommand::Stop => {
                            self.stop_desired_motion(output).await?;
                            self.refresh_active_control_hold(input)?;
                        }
                        BrainCommand::SetMode(mode) => {
                            self.apply_desired_mode(mode, output).await?;
                            self.refresh_active_control_hold(input)?;
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

    fn handle_transport_failure<I, O>(&mut self, input: &mut I, output: &mut O, message: String) -> Result<()>
    where
        I: CommandInputSource,
        O: SessionOutput,
    {
        input.suspend()?;
        self.autonomy.reset();
        self.active_control_state = ControlState::default();
        self.last_desired_state_at = None;
        self.applied_link_timeout_ms = None;
        output.set_connection_status(format!(
            "disconnected; retrying in {} ms",
            self.reconnect_interval.as_millis()
        ))?;
        output.set_control_state(self.active_control_state)?;
        output.log(LogLevel::Warn, message)?;
        self.transport.disconnect();
        Ok(())
    }

    fn handle_telemetry<O>(&mut self, telemetry: Telemetry, output: &mut O) -> Result<()>
    where
        O: SessionOutput,
    {
        output.log(LogLevel::Info, format!("telemetry {}: {telemetry:?}", telemetry.kind()))?;
        self.telemetry.publish(&telemetry);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::{Result, anyhow};
    use mortimmy_core::{Mode, ServoTicks};
    use mortimmy_protocol::messages::{commands::ServoCommand, telemetry::Telemetry};
    use tokio::time::Instant;

    use super::COMMAND_COMPLETION_HOLD_GRACE;

    use crate::{
        brain::{BrainCommand, BrainStep, RobotBrain, transport::{BrainTransport, TransportBackendKind}},
        config::SessionConfig,
        input::{CommandInputSource, ControlState, DriveIntent, InputEvent, ScriptedInput},
        routing::RouterPolicy,
        serial::SerialConfig,
        telemetry::{TelemetryConfig, TelemetryFanout},
        ui::NullSessionOutput,
    };

    #[derive(Default)]
    struct TrackingInput {
        refreshed: Vec<Duration>,
    }

    impl CommandInputSource for TrackingInput {
        fn next_event(&mut self) -> Result<InputEvent> {
            Err(anyhow!("tracking input is eventless"))
        }

        fn extend_active_control(&mut self, duration: Duration) -> Result<()> {
            self.refreshed.push(duration);
            Ok(())
        }
    }

    #[tokio::test]
    async fn ping_roundtrips_over_loopback_transport() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(TransportBackendKind::Loopback, SerialConfig::default(), Duration::from_secs(2)).unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );

        assert_eq!(brain.step(BrainCommand::Ping).await.unwrap(), BrainStep::Continue(Some(Telemetry::Pong)));
    }

    #[tokio::test]
    async fn drive_command_returns_motor_state_telemetry() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(TransportBackendKind::Loopback, SerialConfig::default(), Duration::from_secs(2)).unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );

        match brain.step(BrainCommand::SetMode(Mode::Teleop)).await.unwrap() {
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
            BrainTransport::from_kind(TransportBackendKind::Loopback, SerialConfig::default(), Duration::from_secs(2)).unwrap(),
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

    #[test]
    fn transport_failure_preserves_requested_teleop_mode_for_reconnect() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(TransportBackendKind::Loopback, SerialConfig::default(), Duration::from_secs(2)).unwrap(),
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

    #[test]
    fn reconnect_restores_autonomous_target_after_disconnect() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(TransportBackendKind::Loopback, SerialConfig::default(), Duration::from_secs(2)).unwrap(),
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
            BrainTransport::from_kind(TransportBackendKind::Loopback, SerialConfig::default(), Duration::from_secs(2)).unwrap(),
            TelemetryFanout::new(TelemetryConfig::default()),
            SessionConfig::default(),
        );
        let mut input = ScriptedInput::new([
            InputEvent::Command(BrainCommand::Ping),
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
    async fn run_loop_accepts_combined_drive_and_discrete_command_bursts() {
        let mut brain = RobotBrain::new(
            RouterPolicy::default(),
            BrainTransport::from_kind(TransportBackendKind::Loopback, SerialConfig::default(), Duration::from_secs(2)).unwrap(),
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
            BrainTransport::from_kind(TransportBackendKind::Loopback, SerialConfig::default(), Duration::from_secs(2)).unwrap(),
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
            .handle_input_events(&mut input, &mut output, vec![InputEvent::Command(BrainCommand::Ping)])
            .await
            .unwrap();

        assert!(!should_exit);
        assert_eq!(input.refreshed, vec![COMMAND_COMPLETION_HOLD_GRACE]);
    }
}
