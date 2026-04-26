use std::collections::{BTreeMap, VecDeque};
use std::io::ErrorKind;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use mortimmy_core::Mode;
use serde::Deserialize;
use tokio_tungstenite::tungstenite::{Error as WebsocketError, Message, accept};

use crate::brain::BrainCommand;
use crate::websocket::WebsocketServer;

use super::{
    ControlState, ControllerBackend, ControllerId, ControllerInfo, ControllerKind,
    ControllerLifecycleEvent, DriveIntent, RoutedInputEvent, SourcedInputEvent,
};

const DEFAULT_WEBSOCKET_DRIVE_SPEED: u16 = 300;
const CLIENT_READ_TIMEOUT: Duration = Duration::from_millis(200);
const INPUT_POLL_SLICE: Duration = Duration::from_millis(10);
const LISTENER_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Debug)]
enum WebsocketRuntimeEvent {
    Connected(ControllerInfo),
    Disconnected(ControllerInfo),
    Input(SourcedInputEvent),
}

#[derive(Debug)]
struct WebsocketRuntime {
    event_rx: mpsc::Receiver<WebsocketRuntimeEvent>,
    accepting_input: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    listener_handle: Option<JoinHandle<()>>,
}

impl WebsocketRuntime {
    fn start(bind_address: &str) -> Result<Self> {
        let listener = TcpListener::bind(bind_address).with_context(|| {
            format!("failed to bind websocket controller server to {bind_address}")
        })?;
        listener
            .set_nonblocking(true)
            .context("failed to configure websocket controller listener as non-blocking")?;

        let (event_tx, event_rx) = mpsc::channel();
        let accepting_input = Arc::new(AtomicBool::new(true));
        let shutdown = Arc::new(AtomicBool::new(false));
        let bind_address = bind_address.to_string();
        let listener_accepting_input = Arc::clone(&accepting_input);
        let listener_shutdown = Arc::clone(&shutdown);

        let listener_handle = thread::Builder::new()
            .name("mortimmy-websocket-input".to_string())
            .spawn(move || {
                run_listener(
                    listener,
                    bind_address,
                    event_tx,
                    listener_accepting_input,
                    listener_shutdown,
                );
            })
            .context("failed to start websocket controller listener thread")?;

        Ok(Self {
            event_rx,
            accepting_input,
            shutdown,
            listener_handle: Some(listener_handle),
        })
    }
}

impl Drop for WebsocketRuntime {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);

        if let Some(listener_handle) = self.listener_handle.take() {
            let _ = listener_handle.join();
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum WebsocketClientMessage {
    Control {
        #[serde(default)]
        drive: Option<WebsocketDriveMessage>,
    },
    Command {
        command: WebsocketCommandMessage,
    },
}

#[derive(Debug, Deserialize)]
struct WebsocketDriveMessage {
    forward: f32,
    turn: f32,
    #[serde(default = "default_websocket_drive_speed")]
    speed: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum WebsocketCommandMessage {
    Quit,
    Stop,
    Teleop,
    Autonomous,
    Fault,
}

const fn default_websocket_drive_speed() -> u16 {
    DEFAULT_WEBSOCKET_DRIVE_SPEED
}

#[derive(Debug)]
pub struct WebsocketControllerInput {
    runtime: WebsocketRuntime,
    tracked_controllers: BTreeMap<ControllerId, ControllerInfo>,
    pending_lifecycle: VecDeque<ControllerLifecycleEvent>,
    pending_input: VecDeque<SourcedInputEvent>,
    suspended: bool,
}

impl WebsocketControllerInput {
    pub fn new(server: WebsocketServer) -> Result<Self> {
        let bind_address = server.config.bind_address.clone();
        let runtime = WebsocketRuntime::start(&bind_address)?;

        Ok(Self {
            runtime,
            tracked_controllers: BTreeMap::new(),
            pending_lifecycle: VecDeque::new(),
            pending_input: VecDeque::new(),
            suspended: false,
        })
    }

    fn handle_runtime_event(&mut self, event: WebsocketRuntimeEvent) {
        match event {
            WebsocketRuntimeEvent::Connected(info) => {
                self.tracked_controllers
                    .insert(info.id.clone(), info.clone());
                if !self.suspended {
                    self.pending_lifecycle
                        .push_back(ControllerLifecycleEvent::Connected(info));
                }
            }
            WebsocketRuntimeEvent::Disconnected(info) => {
                self.tracked_controllers.remove(&info.id);
                if !self.suspended {
                    self.pending_lifecycle
                        .push_back(ControllerLifecycleEvent::Disconnected(info));
                }
            }
            WebsocketRuntimeEvent::Input(event) => {
                if !self.suspended {
                    self.pending_input.push_back(event);
                }
            }
        }
    }

    fn drain_runtime_events(&mut self) {
        while let Ok(event) = self.runtime.event_rx.try_recv() {
            self.handle_runtime_event(event);
        }
    }
}

impl ControllerBackend for WebsocketControllerInput {
    fn refresh_controllers(&mut self) -> Result<Vec<ControllerLifecycleEvent>> {
        self.drain_runtime_events();
        Ok(self.pending_lifecycle.drain(..).collect())
    }

    fn poll_input(&mut self, timeout: Duration) -> Result<Option<SourcedInputEvent>> {
        self.drain_runtime_events();
        if let Some(event) = self.pending_input.pop_front() {
            return Ok(Some(event));
        }

        if timeout.is_zero() {
            return Ok(None);
        }

        let started_at = Instant::now();
        while started_at.elapsed() < timeout {
            let wait = timeout
                .saturating_sub(started_at.elapsed())
                .min(INPUT_POLL_SLICE);

            match self.runtime.event_rx.recv_timeout(wait) {
                Ok(event) => {
                    self.handle_runtime_event(event);
                    self.drain_runtime_events();

                    if let Some(event) = self.pending_input.pop_front() {
                        return Ok(Some(event));
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    self.drain_runtime_events();
                    if let Some(event) = self.pending_input.pop_front() {
                        return Ok(Some(event));
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        Ok(None)
    }

    fn suspend(&mut self) -> Result<()> {
        self.suspended = true;
        self.runtime.accepting_input.store(false, Ordering::Relaxed);
        self.pending_lifecycle.clear();
        self.pending_input.clear();
        self.drain_runtime_events();
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        self.suspended = true;
        self.drain_runtime_events();
        self.pending_lifecycle.clear();
        self.pending_input.clear();
        self.runtime.accepting_input.store(true, Ordering::Relaxed);
        self.suspended = false;

        for controller in self.tracked_controllers.values().cloned() {
            self.pending_lifecycle
                .push_back(ControllerLifecycleEvent::Connected(controller));
        }

        Ok(())
    }
}

fn run_listener(
    listener: TcpListener,
    bind_address: String,
    event_tx: mpsc::Sender<WebsocketRuntimeEvent>,
    accepting_input: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) {
    let mut next_connection = 0_u64;

    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, remote_addr)) => {
                next_connection = next_connection.wrapping_add(1);
                spawn_client_thread(
                    stream,
                    remote_addr,
                    next_connection,
                    bind_address.clone(),
                    event_tx.clone(),
                    Arc::clone(&accepting_input),
                    Arc::clone(&shutdown),
                );
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                thread::sleep(LISTENER_POLL_INTERVAL);
            }
            Err(_) => {
                thread::sleep(LISTENER_POLL_INTERVAL);
            }
        }
    }
}

fn spawn_client_thread(
    stream: TcpStream,
    remote_addr: SocketAddr,
    connection_index: u64,
    bind_address: String,
    event_tx: mpsc::Sender<WebsocketRuntimeEvent>,
    accepting_input: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) {
    let _ = thread::Builder::new()
        .name(format!("mortimmy-websocket-client-{connection_index}"))
        .spawn(move || {
            run_client_connection(
                stream,
                remote_addr,
                connection_index,
                bind_address,
                event_tx,
                accepting_input,
                shutdown,
            );
        });
}

fn run_client_connection(
    stream: TcpStream,
    remote_addr: SocketAddr,
    connection_index: u64,
    bind_address: String,
    event_tx: mpsc::Sender<WebsocketRuntimeEvent>,
    accepting_input: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) {
    let _ = stream.set_read_timeout(Some(CLIENT_READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(CLIENT_READ_TIMEOUT));

    let mut websocket = match accept(stream) {
        Ok(websocket) => websocket,
        Err(_) => return,
    };

    let controller = ControllerInfo::new(
        ControllerId::new(
            ControllerKind::Websocket,
            format!("client-{connection_index}"),
        ),
        format!("Websocket {remote_addr} via {bind_address}"),
    );

    if event_tx
        .send(WebsocketRuntimeEvent::Connected(controller.clone()))
        .is_err()
    {
        return;
    }

    while !shutdown.load(Ordering::Relaxed) {
        match websocket.read() {
            Ok(Message::Text(text)) => {
                if !dispatch_text_message(
                    &event_tx,
                    &controller.id,
                    text.as_ref(),
                    accepting_input.load(Ordering::Relaxed),
                ) {
                    break;
                }
            }
            Ok(Message::Binary(bytes)) => {
                let Ok(text) = std::str::from_utf8(bytes.as_ref()) else {
                    continue;
                };

                if !dispatch_text_message(
                    &event_tx,
                    &controller.id,
                    text,
                    accepting_input.load(Ordering::Relaxed),
                ) {
                    break;
                }
            }
            Ok(Message::Ping(payload)) => {
                if websocket.send(Message::Pong(payload)).is_err() {
                    break;
                }
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => break,
            Ok(Message::Frame(_)) => {}
            Err(WebsocketError::Io(error))
                if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                continue;
            }
            Err(WebsocketError::ConnectionClosed | WebsocketError::AlreadyClosed) => break,
            Err(_) => break,
        }
    }

    let _ = event_tx.send(WebsocketRuntimeEvent::Disconnected(controller));
}

fn dispatch_text_message(
    event_tx: &mpsc::Sender<WebsocketRuntimeEvent>,
    controller: &ControllerId,
    text: &str,
    accepting_input: bool,
) -> bool {
    if !accepting_input {
        return true;
    }

    let Ok(events) = parse_websocket_message(text) else {
        return true;
    };

    for event in events {
        if event_tx
            .send(WebsocketRuntimeEvent::Input(SourcedInputEvent::new(
                controller.clone(),
                event,
            )))
            .is_err()
        {
            return false;
        }
    }

    true
}

fn parse_websocket_message(text: &str) -> Result<Vec<RoutedInputEvent>> {
    let message: WebsocketClientMessage =
        serde_json::from_str(text).context("failed to decode websocket control message")?;

    Ok(match message {
        WebsocketClientMessage::Control { drive } => {
            vec![RoutedInputEvent::Control(control_state_from_drive(drive))]
        }
        WebsocketClientMessage::Command { command } => {
            vec![RoutedInputEvent::Command(command.into_brain_command())]
        }
    })
}

fn control_state_from_drive(drive: Option<WebsocketDriveMessage>) -> ControlState {
    ControlState {
        drive: drive.and_then(|drive| {
            let forward = normalized_axis(drive.forward);
            let turn = normalized_axis(drive.turn);

            if forward == 0 && turn == 0 {
                None
            } else {
                Some(DriveIntent {
                    forward,
                    turn,
                    speed: drive.speed.max(1),
                })
            }
        }),
    }
}

fn normalized_axis(value: f32) -> i16 {
    let clamped = value.clamp(-1.0, 1.0);
    (clamped * f32::from(DriveIntent::AXIS_MAX)).round() as i16
}

impl WebsocketCommandMessage {
    fn into_brain_command(self) -> BrainCommand {
        match self {
            Self::Quit => BrainCommand::Quit,
            Self::Stop => BrainCommand::Stop,
            Self::Teleop => BrainCommand::SetMode(Mode::Teleop),
            Self::Autonomous => BrainCommand::SetMode(Mode::Autonomous),
            Self::Fault => BrainCommand::SetMode(Mode::Fault),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::thread;
    use std::time::{Duration, Instant};

    use mortimmy_core::Mode;
    use tokio_tungstenite::tungstenite::{Message, connect};

    use crate::websocket::{WebsocketConfig, WebsocketServer};

    use super::*;

    #[test]
    fn parses_control_messages_into_drive_state() {
        let events = parse_websocket_message(
            r#"{"type":"control","drive":{"forward":0.5,"turn":-0.25,"speed":450}}"#,
        )
        .unwrap();

        assert_eq!(
            events,
            vec![RoutedInputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: 500,
                    turn: -250,
                    speed: 450,
                }),
            })]
        );
    }

    #[test]
    fn parses_command_messages_into_brain_commands() {
        let events =
            parse_websocket_message(r#"{"type":"command","command":"autonomous"}"#).unwrap();

        assert_eq!(
            events,
            vec![RoutedInputEvent::Command(BrainCommand::SetMode(
                Mode::Autonomous
            ))]
        );
    }

    #[test]
    fn websocket_backend_reports_connect_control_command_and_disconnect() {
        let bind_address = reserve_loopback_address();
        let mut backend = WebsocketControllerInput::new(WebsocketServer::new(WebsocketConfig {
            bind_address: bind_address.clone(),
        }))
        .unwrap();

        let websocket_url = format!("ws://{bind_address}");
        let (mut client, _) = connect_with_retry(&websocket_url, Duration::from_secs(1));

        let connected = wait_for_lifecycle_event(&mut backend, Duration::from_secs(1));
        let controller = match connected {
            ControllerLifecycleEvent::Connected(controller) => controller,
            event => panic!("expected websocket connection event, got {event:?}"),
        };

        assert_eq!(controller.id.kind, ControllerKind::Websocket);

        client
            .send(Message::Text(
                r#"{"type":"control","drive":{"forward":1.0,"turn":-0.5,"speed":600}}"#.into(),
            ))
            .unwrap();

        assert_eq!(
            wait_for_input_event(&mut backend, Duration::from_secs(1)),
            SourcedInputEvent::new(
                controller.id.clone(),
                RoutedInputEvent::Control(ControlState {
                    drive: Some(DriveIntent {
                        forward: DriveIntent::AXIS_MAX,
                        turn: -500,
                        speed: 600,
                    }),
                })
            )
        );

        client
            .send(Message::Text(
                r#"{"type":"command","command":"teleop"}"#.into(),
            ))
            .unwrap();

        assert_eq!(
            wait_for_input_event(&mut backend, Duration::from_secs(1)),
            SourcedInputEvent::new(
                controller.id.clone(),
                RoutedInputEvent::Command(BrainCommand::SetMode(Mode::Teleop))
            )
        );

        client.close(None).unwrap();

        assert_eq!(
            wait_for_lifecycle_event(&mut backend, Duration::from_secs(1)),
            ControllerLifecycleEvent::Disconnected(controller)
        );
    }

    fn reserve_loopback_address() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        address.to_string()
    }

    fn connect_with_retry(
        websocket_url: &str,
        timeout: Duration,
    ) -> (
        tokio_tungstenite::tungstenite::WebSocket<
            tokio_tungstenite::tungstenite::stream::MaybeTlsStream<std::net::TcpStream>,
        >,
        tokio_tungstenite::tungstenite::handshake::client::Response,
    ) {
        let started_at = Instant::now();

        loop {
            match connect(websocket_url) {
                Ok(connection) => return connection,
                Err(error) if started_at.elapsed() < timeout => {
                    let is_connection_refused = matches!(
                        error,
                        tokio_tungstenite::tungstenite::Error::Io(ref io_error)
                            if io_error.kind() == ErrorKind::ConnectionRefused
                    );

                    if !is_connection_refused {
                        panic!("failed to connect websocket test client: {error}");
                    }

                    thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("failed to connect websocket test client: {error}"),
            }
        }
    }

    fn wait_for_lifecycle_event(
        backend: &mut WebsocketControllerInput,
        timeout: Duration,
    ) -> ControllerLifecycleEvent {
        let started_at = Instant::now();

        while started_at.elapsed() < timeout {
            let events = backend.refresh_controllers().unwrap();
            if let Some(event) = events.into_iter().next() {
                return event;
            }

            thread::sleep(Duration::from_millis(10));
        }

        panic!("timed out waiting for websocket lifecycle event");
    }

    fn wait_for_input_event(
        backend: &mut WebsocketControllerInput,
        timeout: Duration,
    ) -> SourcedInputEvent {
        let started_at = Instant::now();

        while started_at.elapsed() < timeout {
            if let Some(event) = backend.poll_input(Duration::from_millis(25)).unwrap() {
                return event;
            }
        }

        panic!("timed out waiting for websocket input event");
    }
}
