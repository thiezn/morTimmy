use anyhow::Context;
use cli_helpers::setup_tracing_from_level;
use tokio::time::Duration;

use crate::{
    audio::AudioSubsystem,
    brain::{RobotBrain, command_mapping::RouterPolicy, transport::BrainTransport},
    camera::CameraSubsystem,
    cli::{config::ConfigCommand, start::StartCommand},
    config::{self, AppConfig, LogLevel},
    input::{ControllerSelection, default_controller_registry},
    telemetry::TelemetryFanout,
    tui::{SessionOutput, TuiConfig, new_session},
    websocket::WebsocketServer,
};

pub async fn start(command: StartCommand) -> anyhow::Result<()> {
    let input_backend = command.input_backend;
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
    let serial_target = runtime_config.serial.display_paths();
    let controller_selection_label = match &controller_selection {
        ControllerSelection::Any => "any".to_string(),
        ControllerSelection::Locked(controller) => controller.to_string(),
    };
    let workspace_root = std::env::current_dir().context("failed to determine workspace root")?;
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
        crate::input::InputBackendKind::Tui => {
            let controller_registry =
                default_controller_registry(controller_selection.clone(), websocket.clone())?;
            let (mut input, mut output) = new_session(
                TuiConfig {
                    workspace_root,
                    config_path: config_path.display().to_string(),
                    log_level: runtime_config.logging.level,
                    no_color: runtime_config.logging.no_color,
                    transport_label: format!("{transport_backend:?}"),
                    serial_target: serial_target.clone(),
                    controller_selection: controller_selection_label.clone(),
                    nexo_gateway: runtime_config.nexo.gateway_url.clone(),
                    nexo_client: format!(
                        "{} {} {} {}",
                        runtime_config.nexo.client_id,
                        runtime_config.nexo.client_version,
                        config::nexo_platform_as_str(runtime_config.nexo.platform),
                        runtime_config.nexo.device_id,
                    ),
                    initial_mode: router.default_mode,
                },
                controller_registry,
            )?;
            output.set_connection_status(format!(
                "connecting to {} via {:?}",
                serial_target, transport_backend,
            ))?;
            output.log(
                LogLevel::Info,
                format!(
                    "session starting: config={} baud={} health={}ms reconnect={}ms timeout={}ms nexo_gateway={} telemetry={} audio={} camera={} chunks={} controller_selection={}",
                    config_path.display(),
                    runtime_config.serial.baud_rate,
                    runtime_config.session.health_check_interval_ms,
                    runtime_config.session.reconnect_interval_ms,
                    runtime_config.session.response_timeout_ms,
                    runtime_config.nexo.gateway_url,
                    runtime_config.telemetry.publish_interval_ms,
                    audio.config().enabled,
                    camera.config().enabled,
                    audio_one_second_plan.chunk_count,
                    controller_selection_label,
                ),
            )?;
            brain.run(&mut input, &mut output).await?;
        }
    }

    Ok(())
}

pub fn configure(command: ConfigCommand) -> anyhow::Result<()> {
    let config_path = config::resolve_config_path(command.config.as_deref())?;
    let mut app_config = config::load(&config_path)?;
    command.apply_to(&mut app_config);

    setup_tracing_from_level(app_config.logging.level, app_config.logging.no_color);

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
