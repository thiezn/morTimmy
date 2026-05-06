use anyhow::{Result, anyhow, bail};
use mortimmy_core::{Mode, ServoTicks};
use mortimmy_protocol::messages::{
    ControllerMessage, ControllerResponsePayload, ControlMessage, ReportConfig, ReportKind,
    ReportPayload, RequestPayload,
    commands::ServoCommand,
    telemetry::ControllerCapabilities,
};
use tokio::time::{Duration, Instant};

use crate::{
    audio::{AudioPlanError, AudioSubsystem},
    brain::{
        command_mapping::RouterPolicy,
        transport::{BrainTransport, ConnectedController},
    },
    cli::test::{TestCommand, TestOptions, TestTarget},
    config::{self, AppConfig},
    input::DriveIntent,
};

/// Run the non-TUI validation flow selected by `command`.
pub async fn run(command: TestCommand) -> Result<()> {
    let target = command.selected_target();
    let options = command.options;
    let config_path = config::resolve_config_path(options.config.as_deref())?;
    let runtime_config = load_runtime_config(&config_path, &options)?;
    let audio = AudioSubsystem::from_config(runtime_config.audio.clone());
    let response_timeout = Duration::from_millis(runtime_config.session.response_timeout_ms.max(1));
    let reconnect_interval =
        Duration::from_millis(runtime_config.session.reconnect_interval_ms.max(1));
    let mut transport = BrainTransport::from_kind(
        options.transport_backend,
        runtime_config.serial.clone(),
        response_timeout,
    )?;

    println!(
        "test: connecting target={target:?} transport={:?} devices={} baud={}",
        options.transport_backend,
        runtime_config.serial.display_paths(),
        runtime_config.serial.baud_rate,
    );
    connect_transport(&mut transport, &options, reconnect_interval).await?;
    print_connected_controllers(&transport.connected_controllers());

    let mut harness = TestHarness {
        transport,
        router: RouterPolicy::default(),
        audio,
        options,
        next_control_generation: 1,
    };

    harness.run_target(target).await?;
    println!("test: completed successfully");
    Ok(())
}

/// Load the runtime config from `config_path` and apply CLI overrides from `options`.
fn load_runtime_config(config_path: &std::path::Path, options: &TestOptions) -> Result<AppConfig> {
    let mut runtime_config: AppConfig = config::load_or_create(config_path)?;

    if !options.serial_device.is_empty() {
        runtime_config.serial.device_paths = options.serial_device.clone();
    }
    if let Some(serial_baud_rate) = options.serial_baud_rate {
        runtime_config.serial.baud_rate = serial_baud_rate;
    }
    if let Some(response_timeout_ms) = options.response_timeout_ms {
        runtime_config.session.response_timeout_ms = response_timeout_ms;
    }
    if let Some(reconnect_interval_ms) = options.reconnect_interval_ms {
        runtime_config.session.reconnect_interval_ms = reconnect_interval_ms;
    }

    Ok(runtime_config)
}

/// Connect `transport`, retrying until the timeout configured in `options` expires.
async fn connect_transport(
    transport: &mut BrainTransport,
    options: &TestOptions,
    reconnect_interval: Duration,
) -> Result<()> {
    let connect_deadline = Instant::now() + Duration::from_millis(options.connect_timeout_ms.max(1));

    loop {
        match transport.try_connect().await {
            Ok(()) => return Ok(()),
            Err(error) => {
                if Instant::now() >= connect_deadline {
                    return Err(anyhow!(
                        "timed out waiting for Pico connection after {} ms: {error:#}",
                        options.connect_timeout_ms
                    ));
                }

                println!("test: waiting for Pico connection: {error:#}");
                tokio::time::sleep(reconnect_interval).await;
            }
        }
    }
}

/// Print the discovered `controllers` for the current test session.
fn print_connected_controllers(controllers: &[ConnectedController]) {
    for controller in controllers {
        println!(
            "test: connected controller device={} role={:?} capabilities={:?} mode={:?} error={:?}",
            controller.device_path,
            controller.status.controller_role,
            controller.status.capabilities,
            controller.status.mode,
            controller.status.error,
        );
    }
}

/// Transport-first harness for the `mortimmy test` subcommands.
struct TestHarness {
    transport: BrainTransport,
    router: RouterPolicy,
    audio: AudioSubsystem,
    options: TestOptions,
    next_control_generation: u32,
}

impl TestHarness {
    /// Dispatch to the selected `target` validation flow.
    async fn run_target(&mut self, target: TestTarget) -> Result<()> {
        match target {
            TestTarget::All => self.run_all().await,
            TestTarget::Status => self.run_status().await,
            TestTarget::Drive => self.run_drive().await,
            TestTarget::Servo => self.run_servo().await,
            TestTarget::Sensors => self.run_sensors().await,
            TestTarget::Audio => self.run_audio().await,
        }
    }

    /// Run the full supported validation set for the connected controllers.
    async fn run_all(&mut self) -> Result<()> {
        self.run_status().await?;

        if self.has_capability(ControllerCapabilities::DRIVE) {
            self.run_drive().await?;
        } else {
            println!("test: skipping drive; no controller reports DRIVE capability");
        }

        if self.has_capability(ControllerCapabilities::SERVO) {
            self.run_servo().await?;
        } else {
            println!("test: skipping servo; no controller reports SERVO capability");
        }

        if self.has_capability(ControllerCapabilities::RANGE_SENSOR)
            || self.has_capability(ControllerCapabilities::BATTERY_MONITOR)
        {
            if self.transport.supports_unsolicited_messages() {
                self.run_sensors().await?;
            } else {
                println!(
                    "test: skipping sensors; selected transport does not simulate unsolicited sensor reports"
                );
            }
        } else {
            println!("test: skipping sensors; no controller reports RANGE_SENSOR or BATTERY_MONITOR");
        }

        if self.has_capability(ControllerCapabilities::AUDIO_OUTPUT)
            && self.audio.config().enabled
        {
            self.run_audio().await?;
        } else if self.has_capability(ControllerCapabilities::AUDIO_OUTPUT) {
            println!(
                "test: skipping audio; controller supports audio but host audio forwarding is disabled in config"
            );
        } else {
            println!("test: skipping audio; no controller reports AUDIO_OUTPUT capability");
        }

        self.stop_motion("all:stop").await?;
        self.run_status().await
    }

    /// Request and validate controller status responses.
    async fn run_status(&mut self) -> Result<()> {
        let messages = self
            .request_with_trace("status", RequestPayload::GetControllerStatus)
            .await?;

        if !messages.iter().any(is_controller_status_response) {
            bail!("missing controller status response");
        }

        Ok(())
    }

    /// Exercise latest-wins drive control and validate control-applied reports.
    async fn run_drive(&mut self) -> Result<()> {
        self.require_capability(ControllerCapabilities::DRIVE, "drive")?;
        self.configure_link_timeout().await?;

        let speed = self
            .options
            .drive_speed
            .min(self.router.limits.max_drive_pwm.0 as u16)
            .max(1);
        let script: [(&str, Option<DriveIntent>); 4] = [
            (
                "move-forward",
                Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed,
                }),
            ),
            (
                "move-backward",
                Some(DriveIntent {
                    forward: -DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed,
                }),
            ),
            (
                "turn-left",
                Some(DriveIntent {
                    forward: 0,
                    turn: -DriveIntent::AXIS_MAX,
                    speed,
                }),
            ),
            (
                "turn-right",
                Some(DriveIntent {
                    forward: 0,
                    turn: DriveIntent::AXIS_MAX,
                    speed,
                }),
            ),
        ];

        println!(
            "test: running drive sequence steps={} speed={} emit_interval_ms={} motion_duration_ms={} pause_duration_ms={}",
            script.len(),
            speed,
            self.emit_interval().as_millis(),
            self.motion_duration().as_millis(),
            self.pause_duration().as_millis(),
        );

        for (label, intent) in script {
            self.run_control_window(
                label,
                intent,
                RouterPolicy::centered_servo(),
                self.motion_duration(),
            )
            .await?;
            self.run_control_window(
                &format!("{label}:pause"),
                None,
                RouterPolicy::centered_servo(),
                self.pause_duration(),
            )
            .await?;
        }

        self.stop_motion("drive:stop").await
    }

    /// Exercise servo control without the interactive TUI.
    async fn run_servo(&mut self) -> Result<()> {
        self.require_capability(ControllerCapabilities::SERVO, "servo")?;
        self.configure_link_timeout().await?;

        let step = self.options.servo_step_ticks.max(1);
        let positions = [
            (
                "servo-center",
                ServoCommand {
                    pan: ServoTicks(0),
                    tilt: ServoTicks(0),
                },
            ),
            (
                "servo-pan",
                ServoCommand {
                    pan: ServoTicks(step),
                    tilt: ServoTicks(0),
                },
            ),
            (
                "servo-tilt",
                ServoCommand {
                    pan: ServoTicks(0),
                    tilt: ServoTicks(step),
                },
            ),
            (
                "servo-pan-tilt",
                ServoCommand {
                    pan: ServoTicks(step.saturating_mul(2)),
                    tilt: ServoTicks(step.saturating_mul(2)),
                },
            ),
            (
                "servo-recenter",
                ServoCommand {
                    pan: ServoTicks(0),
                    tilt: ServoTicks(0),
                },
            ),
        ];

        println!(
            "test: running servo sequence steps={} hold_ms={} step_ticks={}",
            positions.len(),
            self.servo_hold_duration().as_millis(),
            step,
        );

        for (label, servo) in positions {
            self.run_control_window(label, None, servo, self.servo_hold_duration())
                .await?;
        }

        self.stop_motion("servo:stop").await
    }

    /// Listen for unsolicited sensor reports and validate the expected report families.
    async fn run_sensors(&mut self) -> Result<()> {
        if !self.transport.supports_unsolicited_messages() {
            bail!(
                "test `sensors` requires a transport that can emit unsolicited controller reports"
            );
        }

        let expects_range = self.has_capability(ControllerCapabilities::RANGE_SENSOR);
        let expects_battery = self.has_capability(ControllerCapabilities::BATTERY_MONITOR);
        if !expects_range && !expects_battery {
            bail!("test `sensors` requires RANGE_SENSOR or BATTERY_MONITOR capability");
        }

        if expects_range {
            self.configure_report(ReportKind::Range).await?;
        }
        if expects_battery {
            self.configure_report(ReportKind::Battery).await?;
        }

        println!(
            "test: listening for sensor traffic for {} ms (range_expected={} battery_expected={})",
            self.sensor_listen_duration().as_millis(),
            expects_range,
            expects_battery,
        );

        let deadline = Instant::now() + self.sensor_listen_duration();
        let mut range_reports = 0usize;
        let mut battery_reports = 0usize;

        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let wait = remaining.min(Duration::from_millis(250));
            let messages = self.drain_with_trace("sensors", wait).await?;

            for message in messages {
                match message {
                    ControllerMessage::Report(report) => match report.payload {
                        ReportPayload::Range(_) => range_reports += 1,
                        ReportPayload::Battery(_) => battery_reports += 1,
                        _ => {}
                    },
                    ControllerMessage::Response(_) | ControllerMessage::Event(_) => {}
                }
            }
        }

        println!(
            "test: sensor summary range_reports={} battery_reports={}",
            range_reports, battery_reports,
        );

        if expects_range && range_reports == 0 {
            bail!("no range sensor reports received within {} ms", self.sensor_listen_duration().as_millis());
        }
        if expects_battery && battery_reports == 0 {
            bail!(
                "no battery monitor reports received within {} ms",
                self.sensor_listen_duration().as_millis()
            );
        }

        Ok(())
    }

    /// Send a short audio waveform and validate the resulting audio responses or reports.
    async fn run_audio(&mut self) -> Result<()> {
        self.require_capability(ControllerCapabilities::AUDIO_OUTPUT, "audio")?;
        if !self.audio.config().enabled {
            bail!(
                "test `audio` requires host audio forwarding to be enabled in config"
            );
        }

        self.configure_report(ReportKind::AudioStatus).await?;

        let channels = usize::from(self.audio.config().channels.max(1));
        let total_samples = ((self.audio.config().sample_rate_hz as usize)
            .saturating_mul(channels)
            .saturating_mul(self.options.audio_duration_ms as usize)
            / 1_000)
            .max(self.audio.config().chunk_samples.max(1));
        let waveform = build_audio_test_waveform(total_samples);
        let plan = self.audio.plan_waveform(waveform.len());
        println!(
            "test: sending audio waveform samples={} chunk_samples={} chunk_count={} duration_ms={}",
            plan.total_samples,
            plan.chunk_samples,
            plan.chunk_count,
            self.options.audio_duration_ms,
        );

        let requests = self
            .audio
            .build_audio_requests(1, &waveform)
            .map_err(audio_plan_error)?;
        let mut saw_audio_response = false;

        for (index, request) in requests.into_iter().enumerate() {
            let label = format!("audio:{}", index + 1);
            let messages = self.request_with_trace(&label, request).await?;
            if messages.iter().any(is_audio_response) {
                saw_audio_response = true;
            }
        }

        let drained = self
            .drain_with_trace("audio:reports", Duration::from_millis(250))
            .await?;
        let saw_audio_status_report = drained.iter().any(|message| {
            matches!(
                message,
                ControllerMessage::Report(report)
                    if matches!(report.payload, ReportPayload::AudioStatus(_))
            )
        });

        if !saw_audio_response && !saw_audio_status_report {
            bail!("audio test did not receive an audio response or audio status report");
        }

        Ok(())
    }

    /// Configure the firmware link timeout to cover the active control emit cadence.
    async fn configure_link_timeout(&mut self) -> Result<()> {
        let desired_timeout_ms = (self.emit_interval().as_millis().saturating_mul(3)).max(250) as u32;
        let messages = self
            .request_with_trace(
                "set-link-timeout",
                RouterPolicy::link_timeout_update(desired_timeout_ms),
            )
            .await?;

        if !messages.iter().any(is_parameter_response) {
            bail!("missing parameter response while setting link timeout");
        }

        Ok(())
    }

    /// Configure the cadence for `report` using the CLI-selected report interval.
    async fn configure_report(&mut self, report: ReportKind) -> Result<()> {
        let messages = self
            .request_with_trace(
                &format!("configure-report:{report:?}"),
                RequestPayload::ConfigureReports(ReportConfig {
                    report,
                    min_interval_ms: self.options.report_interval_ms.max(1),
                    emit_on_change: true,
                }),
            )
            .await?;

        if !messages.iter().any(is_report_config_response) {
            bail!("missing report-config response for {report:?}");
        }

        Ok(())
    }

    /// Send a centered teleop control snapshot and require a control-applied report.
    async fn stop_motion(&mut self, label: &str) -> Result<()> {
        let generation = self.next_generation();
        let control = self
            .router
            .desired_state_command(generation, Mode::Teleop, None, RouterPolicy::centered_servo());
        let messages = self.control_with_trace(label, control).await?;

        if !messages.iter().any(is_control_applied_report) {
            bail!("missing control-applied report while stopping motion");
        }

        Ok(())
    }

    /// Hold `drive` and `servo` for `duration`, validating each emitted control exchange.
    async fn run_control_window(
        &mut self,
        label: &str,
        drive: Option<DriveIntent>,
        servo: ServoCommand,
        duration: Duration,
    ) -> Result<()> {
        let deadline = Instant::now() + duration;
        let mut tick = 0usize;
        let mut saw_control_applied = false;

        while Instant::now() < deadline {
            tick += 1;
            let generation = self.next_generation();
            let control = self.router.desired_state_command(
                generation,
                Mode::Teleop,
                drive,
                servo,
            );
            let messages = self
                .control_with_trace(&format!("{label}:{tick}"), control)
                .await?;

            if messages.iter().any(is_control_applied_report) {
                saw_control_applied = true;
            } else {
                bail!("missing control-applied report during `{label}`");
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            tokio::time::sleep(remaining.min(self.emit_interval())).await;
        }

        if !saw_control_applied {
            bail!("missing control-applied report during `{label}`");
        }

        Ok(())
    }

    /// Send one traced host `request` labelled by `label`.
    async fn request_with_trace(
        &mut self,
        label: &str,
        request: RequestPayload,
    ) -> Result<Vec<ControllerMessage>> {
        println!("tx[{label}]: {:?}", request);
        let messages = self.transport.send_request(request).await?;
        println!("rx[{label}]: {:?}", messages);
        Ok(messages)
    }

    /// Send one traced control `control` labelled by `label`.
    async fn control_with_trace(
        &mut self,
        label: &str,
        control: ControlMessage,
    ) -> Result<Vec<ControllerMessage>> {
        println!("tx[{label}]: {:?}", control);
        let messages = self.transport.send_control(control).await?;
        println!("rx[{label}]: {:?}", messages);
        Ok(messages)
    }

    /// Drain traced controller messages for up to `timeout`.
    async fn drain_with_trace(
        &mut self,
        label: &str,
        timeout: Duration,
    ) -> Result<Vec<ControllerMessage>> {
        let messages = self.transport.drain_messages(timeout).await?;
        if !messages.is_empty() {
            println!("rx[{label}]: {:?}", messages);
        }
        Ok(messages)
    }

    /// Return whether any connected controller exposes `capability`.
    fn has_capability(&self, capability: ControllerCapabilities) -> bool {
        self.transport
            .connected_controllers()
            .into_iter()
            .any(|controller| controller.status.capabilities.contains(capability))
    }

    /// Require at least one controller with `capability` for the test named `label`.
    fn require_capability(&self, capability: ControllerCapabilities, label: &str) -> Result<()> {
        if self.has_capability(capability) {
            Ok(())
        } else {
            bail!("test `{label}` requires a controller with capability {capability:?}")
        }
    }

    /// Return the next control generation and advance the harness counter.
    fn next_generation(&mut self) -> u32 {
        let generation = self.next_control_generation;
        self.next_control_generation = self.next_control_generation.wrapping_add(1);
        generation
    }

    /// Return the desired-state emit interval derived from CLI options.
    fn emit_interval(&self) -> Duration {
        Duration::from_millis(self.options.step_duration_ms.max(1))
    }

    /// Return the drive motion hold duration derived from CLI options.
    fn motion_duration(&self) -> Duration {
        Duration::from_millis(self.options.motion_duration_ms.max(1))
    }

    /// Return the pause duration between drive motions.
    fn pause_duration(&self) -> Duration {
        Duration::from_millis(self.options.pause_duration_ms.max(1))
    }

    /// Return the per-position servo hold duration.
    fn servo_hold_duration(&self) -> Duration {
        Duration::from_millis(self.options.servo_hold_ms.max(1))
    }

    /// Return the sensor listening window derived from CLI options.
    fn sensor_listen_duration(&self) -> Duration {
        Duration::from_millis(self.options.sensor_listen_ms.max(1))
    }
}

/// Build a simple square-wave audio pattern with `total_samples` samples.
fn build_audio_test_waveform(total_samples: usize) -> Vec<i16> {
    let mut waveform = Vec::with_capacity(total_samples);
    for index in 0..total_samples {
        let sample = if index % 32 < 16 { 1_200 } else { -1_200 };
        waveform.push(sample);
    }

    waveform
}

/// Convert an `AudioPlanError` into the user-facing CLI error text.
fn audio_plan_error(error: AudioPlanError) -> anyhow::Error {
    match error {
        AudioPlanError::Disabled => anyhow!("host audio forwarding is disabled"),
        AudioPlanError::UnsupportedBackend => {
            anyhow!("host audio forwarding backend does not emit firmware bridge requests")
        }
        AudioPlanError::ChunkTooLarge => anyhow!("generated audio chunk exceeded protocol capacity"),
        AudioPlanError::TooManyChunks => anyhow!("generated audio waveform required too many chunks"),
    }
}

/// Return whether `message` carries a controller status response.
fn is_controller_status_response(message: &ControllerMessage) -> bool {
    matches!(
        message,
        ControllerMessage::Response(response)
            if matches!(response.payload, ControllerResponsePayload::ControllerStatus(_))
    )
}

/// Return whether `message` carries a parameter update response.
fn is_parameter_response(message: &ControllerMessage) -> bool {
    matches!(
        message,
        ControllerMessage::Response(response)
            if matches!(response.payload, ControllerResponsePayload::Parameter(_))
    )
}

/// Return whether `message` carries a report configuration response.
fn is_report_config_response(message: &ControllerMessage) -> bool {
    matches!(
        message,
        ControllerMessage::Response(response)
            if matches!(response.payload, ControllerResponsePayload::ReportConfig(_))
    )
}

/// Return whether `message` carries an audio request response.
fn is_audio_response(message: &ControllerMessage) -> bool {
    matches!(
        message,
        ControllerMessage::Response(response)
            if matches!(response.payload, ControllerResponsePayload::Audio(_))
    )
}

/// Return whether `message` carries a control-applied report.
fn is_control_applied_report(message: &ControllerMessage) -> bool {
    matches!(
        message,
        ControllerMessage::Report(report)
            if matches!(report.payload, ReportPayload::ControlApplied(_))
    )
}
