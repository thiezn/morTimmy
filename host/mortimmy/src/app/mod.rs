use anyhow::{Context, anyhow, bail};
use mortimmy_protocol::messages::{
    command::Command as ProtocolCommand,
    telemetry::Telemetry,
};
use tokio::time::Duration;

use crate::{
    audio::AudioSubsystem,
    brain::{RobotBrain, transport::BrainTransport},
    camera::CameraSubsystem,
    cli::{config::ConfigCommand, ping::PingCommand, start::StartCommand},
    config::{self, AppConfig, LogLevel},
    input::{CommandInputSource, ControllerSelection, default_controller_registry},
    routing::RouterPolicy,
    telemetry::TelemetryFanout,
    ui::{SessionOutput, SessionUi},
    websocket::WebsocketServer,
};

pub async fn start(command: StartCommand) -> anyhow::Result<()> {
    let input_backend = command.input_backend;
    let keyboard_drive_style = command.keyboard_drive_style;
    let controller_selection = command
        .controller_lock
        .clone()
        .map(ControllerSelection::Locked)
        .unwrap_or_default();
    let transport_backend = command.transport_backend;
    let config_path = config::resolve_config_path(command.config.as_deref())?;
    let file_config = config::load_or_create(&config_path)?;
    let runtime_config = command.merge_config(file_config);

    let audio = AudioSubsystem::from_config(runtime_config.audio.clone());
    let camera = CameraSubsystem::from_config(runtime_config.camera.clone());
    let router = RouterPolicy::default();
    let telemetry = TelemetryFanout::new(runtime_config.telemetry.clone());
    let websocket = WebsocketServer::new(runtime_config.websocket.clone());
    let audio_one_second_plan = audio.plan_waveform(
        runtime_config.audio.sample_rate_hz as usize * usize::from(runtime_config.audio.channels),
    );

    let mut brain = RobotBrain::with_nexo(
        router,
        BrainTransport::from_kind(
            transport_backend,
            runtime_config.serial.clone(),
            Duration::from_millis(runtime_config.session.response_timeout_ms.max(1)),
        )?,
        telemetry,
        runtime_config.session.clone(),
        runtime_config.nexo.clone(),
    );

    match input_backend {
        crate::input::InputBackendKind::Keyboard => {
            let mut input = default_controller_registry(
                controller_selection.clone(),
                keyboard_drive_style,
                websocket.clone(),
            )?;
            let instructions = input.instructions().unwrap_or_default();
            let mut ui = SessionUi::new(
                runtime_config.logging.level,
                runtime_config.logging.no_color,
                instructions.as_ref(),
            )?;
            ui.set_connection_status(format!(
                "connecting to {} via {:?}",
                runtime_config.serial.display_paths(),
                transport_backend,
            ))?;
            ui.log(
                LogLevel::Info,
                format!(
                    "session starting: config={} baud={} health={}ms reconnect={}ms timeout={}ms nexo_gateway={} nexo_client={} nexo_version={} nexo_platform={} nexo_device={} telemetry={} audio={} camera={} chunks={} keyboard_style={} controller_selection={}",
                    config_path.display(),
                    runtime_config.serial.baud_rate,
                    runtime_config.session.health_check_interval_ms,
                    runtime_config.session.reconnect_interval_ms,
                    runtime_config.session.response_timeout_ms,
                    runtime_config.nexo.gateway_url,
                    runtime_config.nexo.client_id,
                    runtime_config.nexo.client_version,
                    config::nexo_platform_as_str(runtime_config.nexo.platform),
                    runtime_config.nexo.device_id,
                    runtime_config.telemetry.publish_interval_ms,
                    audio.config().enabled,
                    camera.config().enabled,
                    audio_one_second_plan.chunk_count,
                    keyboard_drive_style.as_str(),
                    match &controller_selection {
                        ControllerSelection::Any => "any".to_string(),
                        ControllerSelection::Locked(controller) => controller.to_string(),
                    },
                ),
            )?;
            brain.run(&mut input, &mut ui).await?;
        }
    }

    Ok(())
}

pub async fn ping(command: PingCommand) -> anyhow::Result<()> {
    let transport_backend = command.transport_backend;
    let config_path = config::resolve_config_path(command.config.as_deref())?;
    let file_config = config::load_or_create(&config_path)?;
    let runtime_config = command.merge_config(file_config);
    let device_paths = runtime_config.serial.display_paths();

    let mut transport = BrainTransport::from_kind(
        transport_backend,
        runtime_config.serial.clone(),
        Duration::from_millis(runtime_config.session.response_timeout_ms.max(1)),
    )?;

    transport.try_connect().await.map_err(|error| {
        anyhow!(
            "pico ping failed while connecting to {} via {:?}: {error:#}",
            device_paths,
            transport_backend,
        )
    })?;

    match transport.exchange_command(ProtocolCommand::Ping).await.map_err(|error| {
        anyhow!(
            "pico ping failed after connecting to {} via {:?}: {error:#}",
            device_paths,
            transport_backend,
        )
    })? {
        Some(Telemetry::Pong) => {
            let controllers = transport.connected_controllers();

            if controllers.is_empty() {
                bail!("missing status telemetry after ping")
            }

            for controller in controllers {
                println!(
                    "pong device={} transport={transport_backend:?} role={:?} capabilities=0x{:08x}",
                    controller.device_path,
                    controller.status.controller_role,
                    controller.status.capabilities.bits(),
                );
            }

            Ok(())
        }
        Some(telemetry) => bail!("unexpected telemetry after ping: {telemetry:?}"),
        None => bail!("missing telemetry after ping"),
    }
}

pub fn configure(command: ConfigCommand) -> anyhow::Result<()> {
    let config_path = config::resolve_config_path(command.config.as_deref())?;
    let mut app_config = config::load(&config_path)?;
    command.apply_to(&mut app_config);

    setup_tracing(app_config.logging.level, app_config.logging.no_color);

    config::save(&app_config, &config_path)?;
    tracing::info!(config_path = %config_path.display(), "config file updated");

    if command.print {
        print_config(&app_config)?;
    } else {
        println!("Wrote config to {}", config_path.display());
    }

    Ok(())
}

fn print_config(app_config: &AppConfig) -> anyhow::Result<()> {
    let rendered = toml::to_string_pretty(app_config).context("failed to serialize config")?;
    println!("{rendered}");
    Ok(())
}

fn setup_tracing(level: LogLevel, no_color: bool) {
    let filter = if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::EnvFilter::from_default_env()
    } else {
        tracing_subscriber::EnvFilter::new(level.as_str())
    };

    tracing_subscriber::fmt()
        .without_time()
        .with_target(false)
        .with_ansi(!no_color)
        .with_env_filter(filter)
        .init();
}
