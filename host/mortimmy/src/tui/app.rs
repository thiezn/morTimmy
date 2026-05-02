use std::{
    cell::RefCell,
    collections::VecDeque,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

use arboard::Clipboard;
use anyhow::{Context, Result};
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use mortimmy_core::Mode;
use mortimmy_protocol::messages::telemetry::RangeTelemetry;

use crate::{
    brain::BrainCommand,
    config::LogLevel,
    input::{
        CommandInputSource, ControllerId, ControllerInfo, ControllerKind, ControllerRegistry,
        InputEvent, InputWarning,
    },
};

use super::{
    commands::{self, CommandAction, LocalCommand},
    completion,
    files::FileIndex,
    message::Message,
    model::{InputMode, KeyboardDriveState, Model, SummaryStatus, UiLogEntry},
    session::SessionOutput,
    terminal::{self, TuiTerminal},
    update::{self, Action},
    view,
};

const POLL_SLICE: Duration = Duration::from_millis(25);
const TUI_KEYBOARD_CONTROLLER_INSTANCE: &str = "tui-local";
const TUI_KEYBOARD_CONTROLLER_NAME: &str = "TUI Keyboard";

#[derive(Debug, Clone)]
pub struct TuiConfig {
    pub workspace_root: PathBuf,
    pub config_path: String,
    pub log_level: LogLevel,
    pub no_color: bool,
    pub transport_label: String,
    pub serial_target: String,
    pub controller_selection: String,
    pub nexo_gateway: String,
    pub nexo_client: String,
    pub initial_mode: Mode,
}

type SharedRuntime = Rc<RefCell<Runtime>>;

pub struct TuiInput {
    shared: SharedRuntime,
}

pub struct TuiOutput {
    shared: SharedRuntime,
}

struct Runtime {
    terminal: TuiTerminal,
    controller_registry: ControllerRegistry,
    file_index: FileIndex,
    model: Model,
    pending_events: VecDeque<InputEvent>,
}

fn initial_model(config: TuiConfig) -> Model {
    let mut model = Model {
        log_level: config.log_level,
        no_color: config.no_color,
        summary: SummaryStatus {
            config_path: config.config_path,
            connection_status: "connecting".to_string(),
            control_state: Default::default(),
            desired_mode: config.initial_mode,
            distance: None,
            transport_label: config.transport_label,
            serial_target: config.serial_target,
            nexo_gateway: config.nexo_gateway,
            nexo_client: config.nexo_client,
            controller_selection: config.controller_selection,
            active_controllers: Default::default(),
        },
        ..Model::default()
    };
    model.logs.push_back(UiLogEntry {
        level: LogLevel::Info,
        message: "session ready; type /help to view commands".to_string(),
        repeats: 1,
    });
    model
}

pub fn new_session(
    config: TuiConfig,
    controller_registry: ControllerRegistry,
) -> Result<(TuiInput, TuiOutput)> {
    terminal::install_panic_hook();
    let terminal = terminal::init_terminal()?;
    let file_index = FileIndex::discover(&config.workspace_root).with_context(|| {
        format!(
            "failed to index workspace files under {}",
            config.workspace_root.display()
        )
    })?;

    let shared = Rc::new(RefCell::new(Runtime {
        terminal,
        controller_registry,
        file_index,
        model: initial_model(config),
        pending_events: VecDeque::new(),
    }));
    shared.borrow_mut().refresh_completions();
    shared.borrow_mut().render()?;

    Ok((
        TuiInput {
            shared: Rc::clone(&shared),
        },
        TuiOutput { shared },
    ))
}

impl Runtime {
    fn render(&mut self) -> Result<()> {
        self.terminal.draw(|frame| view::view(&mut self.model, frame))?;
        Ok(())
    }

    fn dispatch(&mut self, msg: Message) -> Result<()> {
        match update::update(&mut self.model, msg) {
            Action::None => {}
            Action::ExecuteCommand(command) => self.execute_command(command)?,
            Action::ApplySelectedCompletion => self.apply_selected_completion(),
            Action::CopyAllActivity => self.copy_activity(false)?,
            Action::CopyLastActivity => self.copy_activity(true)?,
        }

        self.refresh_completions();
        self.render()
    }

    fn refresh_completions(&mut self) {
        if self.model.input_mode.keyboard_drive().is_some() {
            self.model.completions.clear();
            self.model.selected_completion = 0;
            return;
        }

        self.model.completions = completion::suggestions(
            &self.model.command_input,
            self.model.cursor,
            &self.file_index,
        );
        if self.model.selected_completion >= self.model.completions.len() {
            self.model.selected_completion = 0;
        }
    }

    fn execute_command(&mut self, command: String) -> Result<()> {
        match commands::parse(&command, &self.file_index) {
            Ok(CommandAction::Emit(event)) => {
                self.pending_events.push_back(event);
            }
            Ok(CommandAction::Local(LocalCommand::Help(topic))) => {
                self.model.help_topic = topic;
                self.model.show_help = true;
            }
            Ok(CommandAction::Local(LocalCommand::EnterKeyboardDrive)) => {
                self.enter_keyboard_drive_mode();
            }
            Err(error) => {
                self.dispatch(Message::Log(
                    LogLevel::Error,
                    format!("command failed: {error:#}"),
                ))?;
            }
        }

        Ok(())
    }

    fn copy_activity(&mut self, last_only: bool) -> Result<()> {
        let maybe_text = if last_only {
            view::last_activity_plain_text(&self.model.logs)
        } else {
            Some(view::activity_plain_text(&self.model.logs))
        };

        let Some(text) = maybe_text.filter(|text| !text.trim().is_empty()) else {
            return self.dispatch(Message::Log(
                LogLevel::Warn,
                "activity is empty; nothing copied".to_string(),
            ));
        };

        match Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text)) {
            Ok(()) => self.dispatch(Message::Log(
                LogLevel::Info,
                if last_only {
                    "copied last activity entry to clipboard".to_string()
                } else {
                    "copied activity log to clipboard".to_string()
                },
            )),
            Err(error) => self.dispatch(Message::Log(
                LogLevel::Error,
                format!("clipboard copy failed: {error}"),
            )),
        }
    }

    fn apply_selected_completion(&mut self) {
        if let Some(suggestion) = self
            .model
            .completions
            .get(self.model.selected_completion)
            .cloned()
        {
            completion::apply_suggestion(
                &mut self.model.command_input,
                &mut self.model.cursor,
                &suggestion,
            );
        }
    }

    fn queue_external_event(&mut self, event: &InputEvent) -> Result<()> {
        match event {
            InputEvent::ControllerConnected(controller) => {
                self.dispatch(Message::ControllerConnected(controller.clone()))?
            }
            InputEvent::ControllerDisconnected(controller) => {
                self.dispatch(Message::ControllerDisconnected(controller.clone()))?
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_terminal_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => self.handle_key_event(key),
            Event::Mouse(mouse) => self.handle_mouse_event(mouse),
            Event::Resize(_, _) => {
                self.terminal.clear()?;
                self.render()
            }
            _ => Ok(()),
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'C' | 'x' | 'X'))
            && matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
        {
            self.pending_events
                .push_back(InputEvent::Command(BrainCommand::Quit));
            return self.render();
        }

        if self.model.input_mode.keyboard_drive().is_some() {
            return self.handle_keyboard_drive_key(key);
        }

        if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return Ok(());
        }

        let message = match key.code {
            KeyCode::Enter => Some(Message::SubmitInput),
            KeyCode::Backspace => Some(Message::Backspace),
            KeyCode::Delete => Some(Message::DeleteForward),
            KeyCode::Left => Some(Message::MoveCursorLeft),
            KeyCode::Right => Some(Message::MoveCursorRight),
            KeyCode::Home => Some(Message::MoveCursorStart),
            KeyCode::End => Some(Message::MoveCursorEnd),
            KeyCode::Tab => Some(Message::ApplySelectedCompletion),
            KeyCode::BackTab => Some(Message::SelectPreviousCompletion),
            KeyCode::Up => Some(Message::SelectPreviousCompletion),
            KeyCode::Down => Some(Message::SelectNextCompletion),
            KeyCode::Esc => Some(Message::ShowHelp(false)),
            KeyCode::F(1) => {
                self.model.help_topic = None;
                Some(Message::ShowHelp(true))
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                Some(Message::InsertChar(character))
            }
            _ => None,
        };

        if let Some(message) = message {
            self.dispatch(message)?;
        }

        Ok(())
    }

    fn handle_keyboard_drive_key(&mut self, key: KeyEvent) -> Result<()> {
        let mut should_render = false;
        match key.kind {
            KeyEventKind::Press | KeyEventKind::Repeat => match key.code {
                KeyCode::Esc | KeyCode::Char('q' | 'Q') => {
                    self.exit_keyboard_drive_mode();
                    should_render = true;
                }
                KeyCode::Char(character) => {
                    let character = character.to_ascii_lowercase();
                    if character == 't' {
                        self.toggle_keyboard_drive_style();
                        should_render = true;
                    } else {
                        should_render = self.apply_keyboard_drive_key(character);
                    }
                }
                _ => {}
            },
            KeyEventKind::Release => {
                if let KeyCode::Char(character) = key.code {
                    should_render = self.apply_keyboard_drive_release_key(character.to_ascii_lowercase());
                }
            }
        }

        if should_render {
            self.refresh_completions();
            self.render()?;
        }

        Ok(())
    }

    fn enter_keyboard_drive_mode(&mut self) {
        if self.model.input_mode.keyboard_drive().is_some() {
            return;
        }

        self.model.input_mode = InputMode::KeyboardDrive(KeyboardDriveState::default());
        self.model.command_input.clear();
        self.model.cursor = 0;
        self.model.show_help = false;
        self.model.help_topic = None;
        self.pending_events
            .push_back(InputEvent::ControllerConnected(tui_keyboard_controller_info()));
        self.pending_events.push_back(InputEvent::Warning(
            InputWarning::Status(
                "TUI keyboard drive active: w,a,s,d mode; press t for tank, Space to stop, Esc or q to exit"
                    .into(),
            ),
        ));
    }

    fn exit_keyboard_drive_mode(&mut self) {
        let Some(state) = self.model.input_mode.keyboard_drive() else {
            return;
        };

        if state.control_state().drive.is_some() {
            self.pending_events
                .push_back(InputEvent::Control(Default::default()));
        }
        self.pending_events.push_back(InputEvent::ControllerDisconnected(
            tui_keyboard_controller_info(),
        ));
        self.pending_events.push_back(InputEvent::Warning(
            InputWarning::Status("TUI keyboard drive exited; command input restored".into()),
        ));
        self.model.input_mode = InputMode::Command;
    }

    fn toggle_keyboard_drive_style(&mut self) {
        let Some(mut state) = self.model.input_mode.keyboard_drive() else {
            return;
        };

        let had_drive = state.control_state().drive.is_some();
        state.toggle_style();
        self.model.input_mode = InputMode::KeyboardDrive(state);
        if had_drive {
            self.pending_events
                .push_back(InputEvent::Control(Default::default()));
        }
        self.pending_events.push_back(InputEvent::Warning(
            InputWarning::Status(
                format!(
                    "TUI keyboard drive style switched to {}",
                    state.style.as_str()
                )
                .into(),
            ),
        ));
    }

    fn apply_keyboard_drive_key(&mut self, key: char) -> bool {
        let Some(mut state) = self.model.input_mode.keyboard_drive() else {
            return false;
        };

        if !state.apply_key_down(key) {
            return false;
        }

        let control_state = state.control_state();
        self.model.input_mode = InputMode::KeyboardDrive(state);
        self.pending_events.push_back(InputEvent::Control(control_state));
        true
    }

    fn apply_keyboard_drive_release_key(&mut self, key: char) -> bool {
        let Some(mut state) = self.model.input_mode.keyboard_drive() else {
            return false;
        };

        if !matches!(key, 'w' | 'a' | 's' | 'd' | 'e' | ' ') {
            return false;
        }

        if !state.apply_key_up(key) {
            return false;
        }

        let control_state = state.control_state();
        self.model.input_mode = InputMode::KeyboardDrive(state);
        self.pending_events.push_back(InputEvent::Control(control_state));
        true
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Result<()> {
        if self.model.show_help {
            return Ok(());
        }

        let size = self.terminal.size()?;
        let layout = view::activity_layout(
            &self.model,
            ratatui::layout::Rect::new(0, 0, size.width, size.height),
        );
        if !contains(layout.panel_area, mouse.column, mouse.row) {
            return Ok(());
        }

        match mouse.kind {
            MouseEventKind::ScrollUp => self.dispatch(Message::ScrollActivityUp),
            MouseEventKind::ScrollDown => self.dispatch(Message::ScrollActivityDown),
            MouseEventKind::Down(MouseButton::Left)
                if contains(layout.copy_all_button_area, mouse.column, mouse.row) =>
            {
                self.dispatch(Message::CopyAllActivity)
            }
            MouseEventKind::Down(MouseButton::Left)
                if contains(layout.copy_last_button_area, mouse.column, mouse.row) =>
            {
                self.dispatch(Message::CopyLastActivity)
            }
            _ => Ok(()),
        }
    }
}

fn contains(rect: ratatui::layout::Rect, column: u16, row: u16) -> bool {
    rect.width > 0
        && rect.height > 0
        && column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn tui_keyboard_controller_info() -> ControllerInfo {
    ControllerInfo::new(
        ControllerId::new(ControllerKind::Keyboard, TUI_KEYBOARD_CONTROLLER_INSTANCE),
        TUI_KEYBOARD_CONTROLLER_NAME,
    )
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = terminal::restore_terminal();
    }
}

impl CommandInputSource for TuiInput {
    fn next_event(&mut self) -> Result<InputEvent> {
        loop {
            if let Some(event) = self.poll_event(Duration::from_millis(250))? {
                return Ok(event);
            }
        }
    }

    fn poll_event(&mut self, timeout: Duration) -> Result<Option<InputEvent>> {
        {
            let mut runtime = self.shared.borrow_mut();
            if let Some(event) = runtime.pending_events.pop_front() {
                runtime.render()?;
                return Ok(Some(event));
            }
            runtime.render()?;
        }

        let started_at = Instant::now();
        loop {
            {
                let mut runtime = self.shared.borrow_mut();
                if let Some(event) = runtime.controller_registry.poll_event(Duration::ZERO)? {
                    runtime.queue_external_event(&event)?;
                    runtime.pending_events.push_back(event);
                }

                if let Some(event) = runtime.pending_events.pop_front() {
                    runtime.render()?;
                    return Ok(Some(event));
                }
            }

            let remaining = timeout.saturating_sub(started_at.elapsed());
            if remaining.is_zero() {
                return Ok(None);
            }

            if event::poll(remaining.min(POLL_SLICE))? {
                let terminal_event = event::read()?;
                let mut runtime = self.shared.borrow_mut();
                runtime.handle_terminal_event(terminal_event)?;
                if let Some(event) = runtime.pending_events.pop_front() {
                    runtime.render()?;
                    return Ok(Some(event));
                }
            }
        }
    }

    fn suspend(&mut self) -> Result<()> {
        self.shared.borrow_mut().controller_registry.suspend()
    }

    fn resume(&mut self) -> Result<()> {
        self.shared.borrow_mut().controller_registry.resume()?;
        self.shared.borrow_mut().render()
    }

    fn extend_active_control(&mut self, duration: Duration) -> Result<()> {
        self.shared
            .borrow_mut()
            .controller_registry
            .extend_active_control(duration)
    }
}

impl SessionOutput for TuiOutput {
    fn log(&mut self, level: LogLevel, message: String) -> Result<()> {
        self.shared.borrow_mut().dispatch(Message::Log(level, message))
    }

    fn set_connection_status(&mut self, status: String) -> Result<()> {
        self.shared
            .borrow_mut()
            .dispatch(Message::SetConnectionStatus(status))
    }

    fn set_control_state(&mut self, control_state: crate::input::ControlState) -> Result<()> {
        self.shared
            .borrow_mut()
            .dispatch(Message::SetControlState(control_state))
    }

    fn set_desired_mode(&mut self, mode: Mode) -> Result<()> {
        self.shared.borrow_mut().dispatch(Message::SetDesiredMode(mode))
    }

    fn set_distance(&mut self, distance: Option<RangeTelemetry>) -> Result<()> {
        self.shared
            .borrow_mut()
            .dispatch(Message::SetDistance(distance))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use mortimmy_core::Mode;

    use super::{TuiConfig, initial_model};
    use crate::config::LogLevel;

    #[test]
    fn initial_model_carries_session_summary_and_banner_log() {
        let model = initial_model(TuiConfig {
            workspace_root: PathBuf::from("/tmp/mortimmy"),
            config_path: "config/mortimmy.toml".to_string(),
            log_level: LogLevel::Debug,
            no_color: true,
            transport_label: "loopback".to_string(),
            serial_target: "/dev/tty.usbmodem".to_string(),
            controller_selection: "any".to_string(),
            nexo_gateway: "ws://127.0.0.1:6969".to_string(),
            nexo_client: "mortimmy".to_string(),
            initial_mode: Mode::Autonomous,
        });

        assert_eq!(model.log_level, LogLevel::Debug);
        assert!(model.no_color);
        assert_eq!(model.summary.config_path, "config/mortimmy.toml");
        assert_eq!(model.summary.connection_status, "connecting");
        assert_eq!(model.summary.desired_mode, Mode::Autonomous);
        assert_eq!(model.summary.distance, None);
        assert_eq!(model.summary.transport_label, "loopback");
        assert_eq!(model.summary.serial_target, "/dev/tty.usbmodem");
        assert_eq!(model.summary.controller_selection, "any");
        assert_eq!(model.summary.nexo_gateway, "ws://127.0.0.1:6969");
        assert_eq!(model.summary.nexo_client, "mortimmy");
        assert!(model.summary.active_controllers.is_empty());
        assert_eq!(model.logs.len(), 1);
        assert_eq!(model.logs[0].level, LogLevel::Info);
        assert_eq!(model.logs[0].message, "session ready; type /help to view commands");
        assert_eq!(model.logs[0].repeats, 1);
    }
}
