#![cfg_attr(all(target_arch = "arm", target_os = "none"), no_std)]

//! Board-aware firmware scaffold for the Pimoroni Pico LiPo 2 and RP2350B.

pub mod actuators;
pub mod board;
pub mod control;
pub mod link_rx;
pub mod link_tx;
pub mod sensors;
pub mod ui;
pub mod usb;

use mortimmy_core::{CoreError, Millimeters, Mode};
use mortimmy_drivers::PadEvent;
use mortimmy_protocol::messages::{
    WireMessage,
    command::Command,
    commands::{ParameterKey, ParameterUpdate},
    telemetry::{
        AudioStatusTelemetry, ControllerCapabilities, ControllerRole, DesiredStateTelemetry,
        StatusTelemetry, Telemetry,
    },
};

const RP2350_BOOTSEL_VOLUME_LABELS: &[&str] = &["RP2350", "RPI-RP2"];
const RP2350_BOOTSEL_INFO_TOKENS: &[&str] = &["RP2350", "RPI-RP2", "Pico"];
const RP2350_BOOTSEL_MANUAL_STEPS: &[&str] = &[
    "Unplug the Pico LiPo 2 from USB-C.",
    "Hold the BOOTSEL button on the board.",
    "Reconnect USB-C while keeping BOOTSEL pressed.",
    "Release BOOTSEL after a mass-storage volume such as RP2350 or RPI-RP2 appears.",
    "Confirm the BOOTSEL device with picotool info or by listing /Volumes.",
];
const MOTION_CONTROLLER_CARGO_FEATURES: &[&str] = &["board-motion-controller"];
const AUDIO_CONTROLLER_CARGO_FEATURES: &[&str] = &["board-audio-controller"];
const DEFAULT_LINK_QUALITY: u8 = 100;

const fn active_controller_role() -> ControllerRole {
    if cfg!(feature = "board-audio-controller") {
        ControllerRole::AudioController
    } else {
        ControllerRole::MotionController
    }
}

const fn active_controller_capabilities() -> ControllerCapabilities {
    let mut bits = 0u32;

    if cfg!(feature = "capability-drive") {
        bits |= ControllerCapabilities::DRIVE.bits();
    }
    if cfg!(feature = "capability-servo") {
        bits |= ControllerCapabilities::SERVO.bits();
    }
    if cfg!(feature = "sensor-ultrasonic") {
        bits |= ControllerCapabilities::RANGE_SENSOR.bits();
    }
    if cfg!(feature = "sensor-battery") {
        bits |= ControllerCapabilities::BATTERY_MONITOR.bits();
    }
    if cfg!(feature = "capability-audio-output") {
        bits |= ControllerCapabilities::AUDIO_OUTPUT.bits();
    }
    if cfg!(feature = "ui-display") {
        bits |= ControllerCapabilities::TEXT_DISPLAY.bits();
    }

    ControllerCapabilities::from_bits(bits)
}

/// Deploy metadata consumed by the host-side tooling.
pub const DEPLOY_TARGET_MOTION_CONTROLLER: mortimmy_deploy::FirmwareTarget =
    mortimmy_deploy::FirmwareTarget {
        id: "motion-controller",
        board_name: "Pimoroni Pico LiPo 2",
        board_mcu: "RP2350B",
        artifact: mortimmy_deploy::Artifact {
            manifest_path: "firmware/rp2350/Cargo.toml",
            package_name: "mortimmy-rp2350",
            bin_name: "mortimmy-rp2350",
            cargo_features: MOTION_CONTROLLER_CARGO_FEATURES,
            cargo_no_default_features: true,
            cargo_target_dir: "target/mortimmy-rp2350-motion-controller",
            target_triple: "thumbv8m.main-none-eabihf",
            default_profile: mortimmy_deploy::BuildProfile::Debug,
        },
        probe: mortimmy_deploy::Probe { chip: "RP235x" },
        uf2: mortimmy_deploy::Uf2 {
            family_name: "RP2350_ARM_S",
            family_id: 0xE48B_FF59,
            absolute_block_location: Some(0x10FF_FF00),
        },
        bootsel: mortimmy_deploy::Bootsel {
            button_name: "BOOTSEL",
            volume_labels: RP2350_BOOTSEL_VOLUME_LABELS,
            info_tokens: RP2350_BOOTSEL_INFO_TOKENS,
            manual_steps: RP2350_BOOTSEL_MANUAL_STEPS,
        },
    };

/// Deploy metadata for the Pico 2 W audio controller image.
pub const DEPLOY_TARGET_AUDIO_CONTROLLER: mortimmy_deploy::FirmwareTarget =
    mortimmy_deploy::FirmwareTarget {
        id: "audio-controller",
        board_name: "Pico 2 W + Pico Audio Pack",
        board_mcu: "RP2350",
        artifact: mortimmy_deploy::Artifact {
            manifest_path: "firmware/rp2350/Cargo.toml",
            package_name: "mortimmy-rp2350",
            bin_name: "mortimmy-rp2350",
            cargo_features: AUDIO_CONTROLLER_CARGO_FEATURES,
            cargo_no_default_features: true,
            cargo_target_dir: "target/mortimmy-rp2350-audio-controller",
            target_triple: "thumbv8m.main-none-eabihf",
            default_profile: mortimmy_deploy::BuildProfile::Debug,
        },
        probe: mortimmy_deploy::Probe { chip: "RP235x" },
        uf2: mortimmy_deploy::Uf2 {
            family_name: "RP2350_ARM_S",
            family_id: 0xE48B_FF59,
            absolute_block_location: Some(0x10FF_FF00),
        },
        bootsel: mortimmy_deploy::Bootsel {
            button_name: "BOOTSEL",
            volume_labels: RP2350_BOOTSEL_VOLUME_LABELS,
            info_tokens: RP2350_BOOTSEL_INFO_TOKENS,
            manual_steps: RP2350_BOOTSEL_MANUAL_STEPS,
        },
    };

#[cfg(all(target_arch = "arm", target_os = "none"))]
use defmt_rtt as _;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_executor::Spawner;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use panic_probe as _;

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[unsafe(link_section = ".start_block")]
#[used]
static IMAGE_DEF: embassy_rp::block::ImageDef = embassy_rp::block::ImageDef::secure_exe();

/// Deterministic firmware bring-up summary shared by host tests and RTT logs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FirmwareBringUpReport {
    /// Human-readable board name.
    pub board_name: &'static str,
    /// Microcontroller identifier.
    pub board_mcu: &'static str,
    /// External flash size in bytes.
    pub flash_bytes: usize,
    /// External PSRAM size in bytes.
    pub psram_bytes: usize,
    /// Default transport path exposed by the firmware.
    pub transport: &'static str,
    /// Safe initial control mode.
    pub control_mode: &'static str,
    /// Default audio routing policy.
    pub audio_route: &'static str,
    /// Default audio chunk size.
    pub audio_chunk_samples: usize,
    /// Whether Trellis polling is enabled at boot.
    pub trellis_enabled: bool,
    /// Whether ultrasonic sensing is enabled at boot.
    pub ultrasonic_enabled: bool,
    /// Whether battery monitoring is enabled at boot.
    pub battery_monitor_enabled: bool,
}

/// Aggregate firmware scaffold state for board bring-up and unit-level testing.
#[derive(Debug)]
pub struct FirmwareScaffold {
    /// Static board profile.
    pub board: board::BoardProfile,
    /// Control loop and safety state.
    pub control: control::ControlLoop,
    /// Link receive path.
    pub link_rx: link_rx::LinkRxTask,
    /// Link transmit path.
    pub link_tx: link_tx::LinkTxTask,
    /// Sensor tasks.
    pub sensors: sensors::SensorSuite,
    /// USB transport setup.
    pub usb: usb::UsbTransport,
    /// Audio output state for the Pico Audio Pack.
    pub audio: ui::audio::AudioOutputTask,
    /// Trellis keypad and LED state.
    pub trellis: ui::trellis::TrellisTask,
}

impl Default for FirmwareScaffold {
    fn default() -> Self {
        Self {
            board: board::active_board_profile(),
            control: control::ControlLoop::new(),
            link_rx: link_rx::LinkRxTask::default(),
            link_tx: link_tx::LinkTxTask::default(),
            sensors: sensors::SensorSuite::default(),
            usb: usb::UsbTransport::new(),
            audio: ui::audio::AudioOutputTask::default(),
            trellis: ui::trellis::TrellisTask::default(),
        }
    }
}

impl FirmwareScaffold {
    /// Summarize the default bring-up state used for logging and tests.
    pub const fn bring_up_report(&self) -> FirmwareBringUpReport {
        FirmwareBringUpReport {
            board_name: self.board.name,
            board_mcu: self.board.mcu,
            flash_bytes: self.board.flash_bytes,
            psram_bytes: self.board.psram_bytes,
            transport: transport_label(self.usb.class),
            control_mode: mode_label(self.control.mode),
            audio_route: audio_route_label(self.audio.route),
            audio_chunk_samples: self.audio.config.chunk_samples,
            trellis_enabled: self.trellis.config.enabled,
            ultrasonic_enabled: self.sensors.ultrasonic.enabled,
            battery_monitor_enabled: self.sensors.battery.enabled,
        }
    }

    /// Apply a protocol command to the scaffold and emit any immediate response telemetry.
    pub fn handle_command(&mut self, command: Command) -> Option<Telemetry> {
        self.link_rx.record_command(&command);

        let response = match command {
            Command::SetDesiredState(desired_state) => {
                self.control.apply_desired_state(desired_state);
                Some(Telemetry::DesiredState(self.desired_state_telemetry()))
            }
            Command::SetParam(update) => {
                self.apply_parameter(update);
                Some(Telemetry::Status(self.status_telemetry()))
            }
            Command::PlayAudio(command) => match self.audio.enqueue_chunk(&command) {
                Ok(()) => {
                    self.link_tx.audio_status_dirty = true;
                    Some(Telemetry::AudioStatus(self.audio_status_telemetry()))
                }
                Err(error) => {
                    self.control.record_error(error);
                    Some(Telemetry::Status(self.status_telemetry()))
                }
            },
            Command::SetTrellisLeds(command) => {
                self.trellis.apply_led_mask(command.led_mask);
                None
            }
            Command::GetStatus => Some(Telemetry::Status(self.status_telemetry())),
        };

        if let Some(telemetry) = response.as_ref() {
            self.link_tx.record_telemetry(telemetry);
        }

        response
    }

    /// Apply a complete wire message and convert any response back into a wire message.
    pub fn apply_wire_message(&mut self, message: WireMessage) -> Option<WireMessage> {
        match message {
            WireMessage::Command(command) => {
                self.handle_command(command).map(WireMessage::Telemetry)
            }
            WireMessage::Telemetry(_) => None,
        }
    }

    /// Snapshot status telemetry from the current control plane.
    pub const fn status_telemetry(&self) -> StatusTelemetry {
        StatusTelemetry {
            mode: self.control.mode,
            controller_role: active_controller_role(),
            capabilities: active_controller_capabilities(),
            uptime_ms: 0,
            link_quality: DEFAULT_LINK_QUALITY,
            error: self.control.last_error,
            range: self.sensors.ultrasonic.last_sample,
        }
    }

    /// Snapshot the applied desired control state.
    pub const fn desired_state_telemetry(&self) -> DesiredStateTelemetry {
        DesiredStateTelemetry::new(
            self.control.mode,
            self.control.drive.telemetry(),
            self.control.servo.telemetry(),
            self.control.last_error,
            self.sensors.ultrasonic.last_sample,
        )
    }

    /// Enter a fault state after link loss or another safety-critical failure.
    pub fn enter_fault_state(&mut self, error: Option<CoreError>) {
        *self = Self::default();
        self.control.mode = Mode::Fault;
        self.control.last_error = error;
    }

    /// Snapshot audio status telemetry from the current playback state.
    pub const fn audio_status_telemetry(&self) -> AudioStatusTelemetry {
        AudioStatusTelemetry {
            queued_chunks: self.audio.queued_chunks,
            speaking: self.audio.queued_chunks > 0,
            underrun_count: 0,
        }
    }

    /// Record a range measurement and return its telemetry representation.
    pub fn record_range_measurement(&mut self, distance_mm: Millimeters, quality: u8) -> Telemetry {
        Telemetry::Range(self.sensors.record_range(distance_mm, quality))
    }

    /// Record a battery measurement and return its telemetry representation.
    pub fn record_battery_measurement(&mut self, millivolts: u16) -> Telemetry {
        Telemetry::Battery(self.sensors.record_battery(millivolts))
    }

    /// Record a Trellis pad event and return its telemetry representation.
    pub fn record_trellis_event(&mut self, event: PadEvent) -> Telemetry {
        self.link_tx.trellis_event_dirty = true;
        let telemetry = Telemetry::TrellisPad(self.trellis.record_pad_event(event));
        self.link_tx.record_telemetry(&telemetry);
        telemetry
    }

    fn apply_parameter(&mut self, update: ParameterUpdate) {
        match self.control.apply_limit_parameter(update) {
            Ok(true) => {}
            Ok(false) => {
                if let Err(error) = self.apply_subsystem_parameter(update) {
                    self.control.record_error(error);
                }
            }
            Err(error) => self.control.record_error(error),
        }
    }

    fn apply_subsystem_parameter(&mut self, update: ParameterUpdate) -> Result<(), CoreError> {
        match update.key {
            ParameterKey::TrellisBrightness => {
                self.trellis.config.brightness =
                    clamp_u8(update.value).ok_or(CoreError::InvalidCommand)?;
                self.control.last_error = None;
                Ok(())
            }
            ParameterKey::TrellisPollIntervalMs => {
                self.trellis.config.poll_interval_ms =
                    clamp_non_zero_u16(update.value).ok_or(CoreError::InvalidCommand)?;
                self.control.last_error = None;
                Ok(())
            }
            ParameterKey::AudioChunkSamples => {
                self.audio.set_chunk_samples(
                    clamp_non_zero_usize(update.value).ok_or(CoreError::InvalidCommand)?,
                )?;
                self.control.last_error = None;
                Ok(())
            }
            ParameterKey::MaxDrivePwm
            | ParameterKey::MaxServoStep
            | ParameterKey::LinkTimeoutMs => Err(CoreError::InvalidCommand),
        }
    }
}

fn clamp_u8(value: i32) -> Option<u8> {
    (0..=i32::from(u8::MAX))
        .contains(&value)
        .then_some(value as u8)
}

fn clamp_non_zero_u16(value: i32) -> Option<u16> {
    (1..=i32::from(u16::MAX))
        .contains(&value)
        .then_some(value as u16)
}

fn clamp_non_zero_usize(value: i32) -> Option<usize> {
    (1..=i32::MAX).contains(&value).then_some(value as usize)
}

const fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Teleop => "teleop",
        Mode::Autonomous => "autonomous",
        Mode::Fault => "fault",
    }
}

const fn transport_label(class: usb::TransportClass) -> &'static str {
    match class {
        usb::TransportClass::UsbCdc => "usb-cdc",
        usb::TransportClass::UartFallback => "uart-fallback",
    }
}

const fn audio_route_label(route: ui::audio::AudioRoute) -> &'static str {
    match route {
        ui::audio::AudioRoute::HostWaveformBridge => "host-waveform-bridge",
        ui::audio::AudioRoute::LocalSynthesis => "local-synthesis",
    }
}

/// Run the firmware on the embedded target.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub async fn run(spawner: Spawner) {
    let scaffold = FirmwareScaffold::default();
    let report = scaffold.bring_up_report();

    defmt::info!(
        "boot board={} mcu={} flash={} psram={} transport={} mode={} audio={} chunk_samples={} trellis={} ultrasonic={} battery={}",
        report.board_name,
        report.board_mcu,
        report.flash_bytes as u32,
        report.psram_bytes as u32,
        report.transport,
        report.control_mode,
        report.audio_route,
        report.audio_chunk_samples as u32,
        report.trellis_enabled,
        report.ultrasonic_enabled,
        report.battery_monitor_enabled,
    );

    usb::run_runtime(spawner).await;
}

/// Run the host-side firmware stub used during local development.
#[cfg(not(all(target_arch = "arm", target_os = "none")))]
pub fn run_host_stub() {
    let scaffold = FirmwareScaffold::default();
    let report = scaffold.bring_up_report();

    println!(
        "mortimmy-rp2350 host stub: board={} mcu={} flash={} psram={} transport={} mode={} audio={} chunk_samples={} trellis={} ultrasonic={} battery={}",
        report.board_name,
        report.board_mcu,
        report.flash_bytes,
        report.psram_bytes,
        report.transport,
        report.control_mode,
        report.audio_route,
        report.audio_chunk_samples,
        report.trellis_enabled,
        report.ultrasonic_enabled,
        report.battery_monitor_enabled,
    );
    println!(
        "Use `cargo embed --chip RP235x --manifest-path firmware/rp2350/Cargo.toml --bin mortimmy-rp2350 --target thumbv8m.main-none-eabihf` when a debug probe is connected, or `cargo check -p mortimmy-rp2350 --target thumbv8m.main-none-eabihf` to validate the embedded target."
    );
}

#[cfg(test)]
mod tests {
    use heapless::Vec;
    use mortimmy_core::Millimeters;
    use mortimmy_drivers::{PadEvent, PadEventKind as DriverPadEventKind, PadIndex};
    use mortimmy_protocol::messages::{
        WireMessage,
        command::Command,
        commands::{
            AUDIO_CHUNK_CAPACITY_SAMPLES, AudioChunkCommand, AudioEncoding, DesiredStateCommand,
            DriveCommand, ParameterKey, ParameterUpdate, ServoCommand,
        },
        telemetry::{ControllerCapabilities, PadEventKind, Telemetry, TrellisPadTelemetry},
    };

    use super::{
        DEPLOY_TARGET_AUDIO_CONTROLLER, DEPLOY_TARGET_MOTION_CONTROLLER, FirmwareScaffold,
        active_controller_capabilities, audio_route_label, mode_label, transport_label,
    };

    #[test]
    fn default_scaffold_reports_safe_bring_up_defaults() {
        let report = FirmwareScaffold::default().bring_up_report();

        assert_eq!(report.board_name, "Pimoroni Pico LiPo 2");
        assert_eq!(report.board_mcu, "RP2350B");
        assert_eq!(report.flash_bytes, 16 * 1024 * 1024);
        assert_eq!(report.psram_bytes, 8 * 1024 * 1024);
        assert_eq!(report.transport, "usb-cdc");
        assert_eq!(report.control_mode, "teleop");
        assert_eq!(report.audio_route, "host-waveform-bridge");
        assert_eq!(report.audio_chunk_samples, 240);
        assert!(!report.trellis_enabled);
        assert!(!report.ultrasonic_enabled);
        assert!(!report.battery_monitor_enabled);
    }

    #[test]
    fn bring_up_labels_are_stable() {
        assert_eq!(mode_label(mortimmy_core::Mode::Teleop), "teleop");
        assert_eq!(
            transport_label(crate::usb::TransportClass::UartFallback),
            "uart-fallback"
        );
        assert_eq!(
            audio_route_label(crate::ui::audio::AudioRoute::LocalSynthesis),
            "local-synthesis"
        );
    }

    #[test]
    fn deploy_metadata_matches_bring_up_defaults() {
        let report = FirmwareScaffold::default().bring_up_report();

        assert_eq!(
            DEPLOY_TARGET_MOTION_CONTROLLER.board_name,
            report.board_name
        );
        assert_eq!(DEPLOY_TARGET_MOTION_CONTROLLER.board_mcu, report.board_mcu);
        assert_eq!(
            DEPLOY_TARGET_MOTION_CONTROLLER.artifact.manifest_path,
            "firmware/rp2350/Cargo.toml"
        );
        assert_eq!(
            DEPLOY_TARGET_MOTION_CONTROLLER.artifact.cargo_features,
            &["board-motion-controller"]
        );
        assert!(core::hint::black_box(
            DEPLOY_TARGET_MOTION_CONTROLLER
                .artifact
                .cargo_no_default_features
        ));
        assert_eq!(
            DEPLOY_TARGET_MOTION_CONTROLLER.artifact.cargo_target_dir,
            "target/mortimmy-rp2350-motion-controller"
        );
        assert_eq!(
            DEPLOY_TARGET_MOTION_CONTROLLER.artifact.target_triple,
            "thumbv8m.main-none-eabihf"
        );
        assert_eq!(DEPLOY_TARGET_MOTION_CONTROLLER.probe.chip, "RP235x");
        assert_eq!(
            DEPLOY_TARGET_MOTION_CONTROLLER.uf2.family_name,
            "RP2350_ARM_S"
        );
        assert_eq!(DEPLOY_TARGET_MOTION_CONTROLLER.uf2.family_id, 0xE48B_FF59);
        assert_eq!(
            DEPLOY_TARGET_MOTION_CONTROLLER.uf2.absolute_block_location,
            Some(0x10FF_FF00)
        );
        assert_eq!(
            DEPLOY_TARGET_MOTION_CONTROLLER.bootsel.button_name,
            "BOOTSEL"
        );
        assert!(
            DEPLOY_TARGET_MOTION_CONTROLLER
                .bootsel
                .volume_labels
                .contains(&"RP2350")
        );
    }

    #[test]
    fn deploy_targets_keep_controller_feature_bundles_isolated() {
        assert_eq!(DEPLOY_TARGET_MOTION_CONTROLLER.id, "motion-controller");
        assert_eq!(DEPLOY_TARGET_AUDIO_CONTROLLER.id, "audio-controller");
        assert_eq!(
            DEPLOY_TARGET_MOTION_CONTROLLER.artifact.cargo_features,
            &["board-motion-controller"]
        );
        assert_eq!(
            DEPLOY_TARGET_AUDIO_CONTROLLER.artifact.cargo_features,
            &["board-audio-controller"]
        );
        assert_ne!(
            DEPLOY_TARGET_MOTION_CONTROLLER.artifact.cargo_target_dir,
            DEPLOY_TARGET_AUDIO_CONTROLLER.artifact.cargo_target_dir
        );
    }

    #[test]
    fn motion_controller_feature_bundle_matches_motor_and_ultrasonic_scope() {
        let capabilities = active_controller_capabilities();

        assert!(capabilities.contains(ControllerCapabilities::DRIVE));
        assert!(capabilities.contains(ControllerCapabilities::SERVO));
        assert!(capabilities.contains(ControllerCapabilities::RANGE_SENSOR));
        assert!(!capabilities.contains(ControllerCapabilities::BATTERY_MONITOR));
        assert!(!capabilities.contains(ControllerCapabilities::AUDIO_OUTPUT));
        assert!(!capabilities.contains(ControllerCapabilities::TEXT_DISPLAY));
    }

    #[test]
    fn desired_state_command_updates_control_state_and_emits_combined_telemetry() {
        let mut scaffold = FirmwareScaffold::default();

        let response = scaffold.handle_command(Command::SetDesiredState(DesiredStateCommand::new(
            mortimmy_core::Mode::Teleop,
            DriveCommand {
                left: mortimmy_core::PwmTicks(320),
                right: mortimmy_core::PwmTicks(-240),
            },
            ServoCommand {
                pan: mortimmy_core::ServoTicks(24),
                tilt: mortimmy_core::ServoTicks(36),
            },
        )));

        assert_eq!(
            scaffold.link_rx.last_command_kind,
            Some("set-desired-state")
        );
        assert_eq!(scaffold.link_tx.last_telemetry_kind, Some("desired-state"));
        assert_eq!(scaffold.control.mode, mortimmy_core::Mode::Teleop);
        assert_eq!(
            response,
            Some(Telemetry::DesiredState(scaffold.desired_state_telemetry()))
        );
    }

    #[test]
    fn status_and_desired_state_telemetry_include_latest_range_sample() {
        let mut scaffold = FirmwareScaffold::default();
        scaffold.record_range_measurement(Millimeters(287), 100);

        assert_eq!(
            scaffold.status_telemetry().range,
            Some(mortimmy_protocol::messages::telemetry::RangeTelemetry {
                distance_mm: Millimeters(287),
                quality: 100,
            })
        );
        assert_eq!(
            scaffold.desired_state_telemetry().range,
            Some(mortimmy_protocol::messages::telemetry::RangeTelemetry {
                distance_mm: Millimeters(287),
                quality: 100,
            })
        );
    }

    #[test]
    fn enter_fault_state_clears_control_audio_and_trellis() {
        let mut scaffold = FirmwareScaffold::default();
        scaffold.handle_command(Command::SetDesiredState(DesiredStateCommand::new(
            mortimmy_core::Mode::Teleop,
            DriveCommand {
                left: mortimmy_core::PwmTicks(200),
                right: mortimmy_core::PwmTicks(200),
            },
            ServoCommand {
                pan: mortimmy_core::ServoTicks(24),
                tilt: mortimmy_core::ServoTicks(36),
            },
        )));
        scaffold.audio.queued_chunks = 2;
        scaffold.trellis.apply_led_mask(0x00ff);

        scaffold.enter_fault_state(Some(mortimmy_core::CoreError::LinkTimedOut));

        assert_eq!(scaffold.control.mode, mortimmy_core::Mode::Fault);
        assert_eq!(scaffold.control.drive.left_pwm.0, 0);
        assert_eq!(scaffold.control.drive.right_pwm.0, 0);
        assert_eq!(scaffold.audio.queued_chunks, 0);
        assert_eq!(scaffold.trellis.led_mask, 0);
        assert_eq!(
            scaffold.control.last_error,
            Some(mortimmy_core::CoreError::LinkTimedOut)
        );
    }

    #[test]
    fn parameter_updates_reconfigure_subsystems() {
        let mut scaffold = FirmwareScaffold::default();

        scaffold.handle_command(Command::SetParam(ParameterUpdate {
            key: ParameterKey::AudioChunkSamples,
            value: 120,
        }));
        scaffold.handle_command(Command::SetParam(ParameterUpdate {
            key: ParameterKey::TrellisBrightness,
            value: 48,
        }));

        assert_eq!(scaffold.audio.config.chunk_samples, 120);
        assert_eq!(scaffold.trellis.config.brightness, 48);
        assert_eq!(scaffold.control.last_error, None);
    }

    #[test]
    fn invalid_parameter_update_surfaces_status_error() {
        let mut scaffold = FirmwareScaffold::default();

        let response = scaffold.handle_command(Command::SetParam(ParameterUpdate {
            key: ParameterKey::AudioChunkSamples,
            value: (AUDIO_CHUNK_CAPACITY_SAMPLES + 1) as i32,
        }));

        assert_eq!(
            scaffold.control.last_error,
            Some(mortimmy_core::CoreError::InvalidCommand)
        );
        assert_eq!(
            response,
            Some(Telemetry::Status(scaffold.status_telemetry()))
        );
    }

    #[test]
    fn full_size_audio_chunk_is_accepted_and_reported() {
        let mut scaffold = FirmwareScaffold::default();
        let mut samples = Vec::<i16, AUDIO_CHUNK_CAPACITY_SAMPLES>::new();
        for sample in 0..AUDIO_CHUNK_CAPACITY_SAMPLES {
            samples.push(sample as i16).unwrap();
        }

        let response = scaffold.handle_command(Command::PlayAudio(AudioChunkCommand {
            utterance_id: 1,
            chunk_index: 0,
            sample_rate_hz: 24_000,
            channels: 1,
            encoding: AudioEncoding::SignedPcm16Le,
            is_final_chunk: true,
            samples,
        }));

        assert_eq!(scaffold.audio.queued_chunks, 1);
        assert_eq!(
            response,
            Some(Telemetry::AudioStatus(scaffold.audio_status_telemetry()))
        );
    }

    #[test]
    fn wire_message_status_command_roundtrips_to_status_response() {
        let mut scaffold = FirmwareScaffold::default();

        let response = scaffold.apply_wire_message(WireMessage::Command(Command::GetStatus));

        assert_eq!(
            response,
            Some(WireMessage::Telemetry(Telemetry::Status(
                scaffold.status_telemetry()
            )))
        );
    }

    #[test]
    fn trellis_events_are_converted_into_protocol_telemetry() {
        let mut scaffold = FirmwareScaffold::default();

        let telemetry = scaffold.record_trellis_event(PadEvent {
            index: PadIndex::new(4).unwrap(),
            kind: DriverPadEventKind::Pressed,
        });

        assert_eq!(
            telemetry,
            Telemetry::TrellisPad(TrellisPadTelemetry {
                pad_index: 4,
                event: PadEventKind::Pressed,
            })
        );
        assert_eq!(scaffold.link_tx.last_telemetry_kind, Some("trellis-pad"));
    }
}
