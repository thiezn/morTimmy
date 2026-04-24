use std::borrow::Cow;
use std::collections::VecDeque;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::ValueEnum;
use crossterm::{
    event::{
        self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement},
};
use mortimmy_core::Mode;

use crate::brain::BrainCommand;

use super::{
    ControllerBackend, ControllerId, ControllerInfo, ControllerKind, ControllerLifecycleEvent,
    RoutedInputEvent, SourcedInputEvent,
    source::{CommandInputSource, ControlState, DriveIntent, InputEvent, InputWarning},
};

const KEYBOARD_DRIVE_SPEED: u16 = 300;
const INITIAL_DRIVE_KEY_LEASE: Duration = Duration::from_millis(750);
const DRIVE_KEY_LEASE: Duration = Duration::from_millis(350);
const INTERLEAVED_KEY_LEASE: Duration = Duration::from_millis(1_000);
const TOGGLE_DRIVE_STYLE_KEY: char = 'm';
const CHAT_MODE_KEY: char = 'c';

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum KeyboardDriveStyle {
    #[default]
    Wasd,
    Tank,
}

impl KeyboardDriveStyle {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wasd => "wasd",
            Self::Tank => "tank",
        }
    }

    const fn help_text(self) -> &'static str {
        match self {
            Self::Wasd => {
                "Keyboard commands (wasd):\n  hold w | up           Drive forward while held\n  hold s | down         Drive backward while held\n  hold a | left         Turn left while held\n  hold d | right        Turn right while held\n  m                     Switch to tank mode\n  c                     Open nexo chat prompt\n  p                     Send a ping and expect pong telemetry\n  x | space             Stop the robot\n  t                     Switch to teleop mode\n  u                     Switch to autonomous mode (default servo-scan plan)\n  f                     Switch to fault mode\n  q | Ctrl-C            Exit the loop\n"
            }
            Self::Tank => {
                "Keyboard commands (tank):\n  hold w                Left tread forward\n  hold s                Left tread reverse\n  hold e                Right tread forward\n  hold d                Right tread reverse\n  hold a                Pivot left (left forward, right reverse)\n  m                     Switch to wasd mode\n  c                     Open nexo chat prompt\n  p                     Send a ping and expect pong telemetry\n  x | space             Stop the robot\n  t                     Switch to teleop mode\n  u                     Switch to autonomous mode (default servo-scan plan)\n  f                     Switch to fault mode\n  q | Ctrl-C            Exit the loop\n"
            }
        }
    }
}

fn drive_key_lease(kind: KeyEventKind) -> Option<Duration> {
    match kind {
        KeyEventKind::Press => Some(INITIAL_DRIVE_KEY_LEASE),
        KeyEventKind::Repeat => Some(DRIVE_KEY_LEASE),
        KeyEventKind::Release => None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct DriveKeyState {
    pressed: bool,
    deadline: Option<Instant>,
}

impl DriveKeyState {
    fn is_active(self, now: Instant) -> bool {
        self.pressed || self.deadline.is_some_and(|deadline| deadline > now)
    }

    fn apply(&mut self, kind: KeyEventKind, now: Instant, track_pressed_state: bool) {
        match kind {
            KeyEventKind::Press => {
                if track_pressed_state {
                    self.pressed = true;
                }
                self.deadline = Some(now + INITIAL_DRIVE_KEY_LEASE);
            }
            KeyEventKind::Repeat => {
                if track_pressed_state {
                    self.pressed = true;
                }
                self.deadline = Some(now + DRIVE_KEY_LEASE);
            }
            KeyEventKind::Release => {
                self.pressed = false;
                self.deadline = None;
            }
        }
    }

    fn refresh_active(&mut self, now: Instant, lease: Duration) {
        if self.is_active(now) {
            self.deadline = Some(now + lease);
        }
    }

    fn refresh_tracked(&mut self, now: Instant, lease: Duration) {
        if self.pressed || self.deadline.is_some() {
            self.deadline = Some(now + lease);
        }
    }

    fn expire(&mut self, now: Instant) -> bool {
        let expired = self.deadline.is_some_and(|deadline| deadline <= now);

        if expired {
            self.deadline = None;
        }

        expired && !self.pressed
    }

    fn next_expiration(self, now: Instant) -> Option<Duration> {
        if self.pressed {
            return None;
        }

        self.deadline
            .filter(|deadline| *deadline > now)
            .map(|deadline| deadline.saturating_duration_since(now))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct HeldDriveKeys {
    forward: DriveKeyState,
    backward: DriveKeyState,
    left: DriveKeyState,
    right: DriveKeyState,
    left_track_forward: DriveKeyState,
    left_track_backward: DriveKeyState,
    right_track_forward: DriveKeyState,
    right_track_backward: DriveKeyState,
    tank_pivot_left: DriveKeyState,
}

impl HeldDriveKeys {
    fn axis_value(positive: bool, negative: bool) -> i16 {
        match (positive, negative) {
            (true, false) => DriveIntent::AXIS_MAX,
            (false, true) => -DriveIntent::AXIS_MAX,
            _ => 0,
        }
    }

    fn is_drive_key(style: KeyboardDriveStyle, code: KeyCode) -> bool {
        match style {
            KeyboardDriveStyle::Wasd => matches!(
                code,
                KeyCode::Up
                    | KeyCode::Char('w')
                    | KeyCode::Char('W')
                    | KeyCode::Down
                    | KeyCode::Char('s')
                    | KeyCode::Char('S')
                    | KeyCode::Left
                    | KeyCode::Char('a')
                    | KeyCode::Char('A')
                    | KeyCode::Right
                    | KeyCode::Char('d')
                    | KeyCode::Char('D')
            ),
            KeyboardDriveStyle::Tank => matches!(
                code,
                KeyCode::Char('w')
                    | KeyCode::Char('W')
                    | KeyCode::Char('s')
                    | KeyCode::Char('S')
                    | KeyCode::Char('e')
                    | KeyCode::Char('E')
                    | KeyCode::Char('d')
                    | KeyCode::Char('D')
                    | KeyCode::Char('a')
                    | KeyCode::Char('A')
            ),
        }
    }

    fn apply(
        &mut self,
        style: KeyboardDriveStyle,
        code: KeyCode,
        kind: KeyEventKind,
        now: Instant,
        track_pressed_state: bool,
    ) -> bool {
        let slot = match style {
            KeyboardDriveStyle::Wasd => match code {
                KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') => &mut self.forward,
                KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') => &mut self.backward,
                KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => &mut self.left,
                KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => &mut self.right,
                _ => return false,
            },
            KeyboardDriveStyle::Tank => match code {
                KeyCode::Char('w') | KeyCode::Char('W') => &mut self.left_track_forward,
                KeyCode::Char('s') | KeyCode::Char('S') => &mut self.left_track_backward,
                KeyCode::Char('e') | KeyCode::Char('E') => &mut self.right_track_forward,
                KeyCode::Char('d') | KeyCode::Char('D') => &mut self.right_track_backward,
                KeyCode::Char('a') | KeyCode::Char('A') => &mut self.tank_pivot_left,
                _ => return false,
            },
        };

        let was_active = slot.is_active(now);
        slot.apply(kind, now, track_pressed_state);
        was_active != slot.is_active(now)
    }

    fn refresh_active(&mut self, now: Instant, lease: Duration) {
        for slot in [
            &mut self.forward,
            &mut self.backward,
            &mut self.left,
            &mut self.right,
            &mut self.left_track_forward,
            &mut self.left_track_backward,
            &mut self.right_track_forward,
            &mut self.right_track_backward,
            &mut self.tank_pivot_left,
        ] {
            slot.refresh_active(now, lease);
        }
    }

    fn refresh_tracked(&mut self, now: Instant, lease: Duration) {
        for slot in [
            &mut self.forward,
            &mut self.backward,
            &mut self.left,
            &mut self.right,
            &mut self.left_track_forward,
            &mut self.left_track_backward,
            &mut self.right_track_forward,
            &mut self.right_track_backward,
            &mut self.tank_pivot_left,
        ] {
            slot.refresh_tracked(now, lease);
        }
    }

    fn expire(&mut self, now: Instant) -> bool {
        let mut changed = false;

        for slot in [
            &mut self.forward,
            &mut self.backward,
            &mut self.left,
            &mut self.right,
            &mut self.left_track_forward,
            &mut self.left_track_backward,
            &mut self.right_track_forward,
            &mut self.right_track_backward,
            &mut self.tank_pivot_left,
        ] {
            if slot.expire(now) {
                changed = true;
            }
        }

        changed
    }

    fn next_expiration(self, now: Instant) -> Option<Duration> {
        [
            self.forward.next_expiration(now),
            self.backward.next_expiration(now),
            self.left.next_expiration(now),
            self.right.next_expiration(now),
            self.left_track_forward.next_expiration(now),
            self.left_track_backward.next_expiration(now),
            self.right_track_forward.next_expiration(now),
            self.right_track_backward.next_expiration(now),
            self.tank_pivot_left.next_expiration(now),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    fn to_control_state(self, style: KeyboardDriveStyle, now: Instant) -> ControlState {
        let (forward, turn) = match style {
            KeyboardDriveStyle::Wasd => {
                let forward =
                    Self::axis_value(self.forward.is_active(now), self.backward.is_active(now));
                let turn = Self::axis_value(self.right.is_active(now), self.left.is_active(now));
                (forward, turn)
            }
            KeyboardDriveStyle::Tank => {
                let tank_pivot_left = self.tank_pivot_left.is_active(now);
                let left_track = Self::axis_value(
                    self.left_track_forward.is_active(now) || tank_pivot_left,
                    self.left_track_backward.is_active(now),
                );
                let right_track = Self::axis_value(
                    self.right_track_forward.is_active(now),
                    self.right_track_backward.is_active(now) || tank_pivot_left,
                );
                let forward = ((i32::from(left_track) + i32::from(right_track)) / 2) as i16;
                let turn = ((i32::from(right_track) - i32::from(left_track)) / 2) as i16;
                (forward, turn)
            }
        };

        let drive = if forward == 0 && turn == 0 {
            None
        } else {
            Some(DriveIntent {
                forward,
                turn,
                speed: KEYBOARD_DRIVE_SPEED,
            })
        };

        ControlState { drive }
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Line-oriented keyboard input backend for local robot bring-up.
#[derive(Debug, Default)]
pub struct KeyboardInput {
    drive_style: KeyboardDriveStyle,
    controller_connected: bool,
    raw_mode_enabled: bool,
    keyboard_enhancement_enabled: bool,
    chat_buffer: Option<String>,
    held_drive_keys: HeldDriveKeys,
    current_control_state: ControlState,
    pending_events: VecDeque<InputEvent>,
}

impl KeyboardInput {
    /// Construct the keyboard input backend.
    pub fn new() -> Self {
        Self::with_drive_style(KeyboardDriveStyle::default())
    }

    pub fn with_drive_style(drive_style: KeyboardDriveStyle) -> Self {
        Self {
            drive_style,
            controller_connected: false,
            raw_mode_enabled: false,
            keyboard_enhancement_enabled: false,
            chat_buffer: None,
            held_drive_keys: HeldDriveKeys::default(),
            current_control_state: ControlState::default(),
            pending_events: VecDeque::new(),
        }
    }

    /// Human-readable keyboard command reference.
    pub const fn help_text(style: KeyboardDriveStyle) -> &'static str {
        style.help_text()
    }

    fn help_text_for_current_style(&self) -> &'static str {
        Self::help_text(self.drive_style)
    }

    fn toggle_drive_style(&mut self, now: Instant) {
        self.drive_style = match self.drive_style {
            KeyboardDriveStyle::Wasd => KeyboardDriveStyle::Tank,
            KeyboardDriveStyle::Tank => KeyboardDriveStyle::Wasd,
        };
        self.held_drive_keys.reset();
        self.sync_control_state(now);
        self.enqueue_input_event(InputEvent::Warning(InputWarning::Status(Cow::Owned(
            format!(
                "keyboard drive style switched to {}",
                self.drive_style.as_str()
            ),
        ))));
    }

    fn enter_chat_mode(&mut self, now: Instant) {
        if self.chat_buffer.is_some() {
            return;
        }

        self.chat_buffer = Some(String::new());
        self.held_drive_keys.reset();
        self.sync_control_state(now);
        self.enqueue_input_event(InputEvent::Prompt(Some("chat> ".to_string())));
        self.enqueue_input_event(InputEvent::Warning(InputWarning::Status(Cow::Borrowed(
            "chat mode active: type a message and press Enter",
        ))));
    }

    fn update_chat_prompt(&mut self) {
        let prompt = self
            .chat_buffer
            .as_deref()
            .map(|chat_buffer| format!("chat> {chat_buffer}"));
        self.enqueue_input_event(InputEvent::Prompt(prompt));
    }

    fn leave_chat_mode(&mut self) {
        self.chat_buffer = None;
        self.enqueue_input_event(InputEvent::Prompt(None));
    }

    fn handle_chat_key_event(&mut self, event: KeyEvent) -> Result<()> {
        if !matches!(event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return Ok(());
        }

        match event.code {
            KeyCode::Esc => {
                self.leave_chat_mode();
                self.enqueue_input_event(InputEvent::Warning(InputWarning::Status(
                    Cow::Borrowed("chat mode cancelled"),
                )));
            }
            KeyCode::Enter => {
                let prompt = self.chat_buffer.take().unwrap_or_default();
                self.enqueue_input_event(InputEvent::Prompt(None));

                let prompt = prompt.trim().to_string();
                if prompt.is_empty() {
                    self.enqueue_input_event(InputEvent::Warning(InputWarning::Status(
                        Cow::Borrowed("chat mode cancelled: prompt was empty"),
                    )));
                } else {
                    self.enqueue_input_event(InputEvent::Command(BrainCommand::Chat(prompt)));
                }
            }
            KeyCode::Backspace => {
                if let Some(chat_buffer) = self.chat_buffer.as_mut() {
                    chat_buffer.pop();
                }
                self.update_chat_prompt();
            }
            KeyCode::Char(character)
                if !event
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if let Some(chat_buffer) = self.chat_buffer.as_mut() {
                    chat_buffer.push(character);
                }
                self.update_chat_prompt();
            }
            _ => {}
        }

        Ok(())
    }

    fn controller_info() -> ControllerInfo {
        ControllerInfo::new(
            ControllerId::new(ControllerKind::Keyboard, "local"),
            "Local Keyboard",
        )
    }

    fn ensure_raw_mode(&mut self) -> Result<()> {
        if self.raw_mode_enabled {
            return Ok(());
        }

        enable_raw_mode()?;
        self.raw_mode_enabled = true;

        if supports_keyboard_enhancement().unwrap_or(false) {
            let mut stdout = io::stdout();
            if execute!(
                stdout,
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                )
            )
            .is_ok()
            {
                self.keyboard_enhancement_enabled = true;
            }
        }

        Ok(())
    }

    fn disable_raw_mode_if_enabled(&mut self) -> Result<()> {
        if !self.raw_mode_enabled {
            return Ok(());
        }

        if self.keyboard_enhancement_enabled {
            let mut stdout = io::stdout();
            let _ = execute!(stdout, PopKeyboardEnhancementFlags);
            self.keyboard_enhancement_enabled = false;
        }

        disable_raw_mode()?;
        self.raw_mode_enabled = false;
        Ok(())
    }

    fn drain_pending_events(&mut self) -> Result<()> {
        while event::poll(Duration::ZERO)? {
            let _ = event::read()?;
        }

        self.pending_events.clear();
        self.current_control_state = ControlState::default();

        Ok(())
    }

    fn key_event_to_discrete_event(
        style: KeyboardDriveStyle,
        event: KeyEvent,
    ) -> Option<InputEvent> {
        if !matches!(event.kind, KeyEventKind::Press) {
            return None;
        }

        if event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(event.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            return Some(InputEvent::Command(BrainCommand::Quit));
        }

        match event.code {
            KeyCode::Char(' ') => Some(InputEvent::Command(BrainCommand::Stop)),
            KeyCode::Char(character) => {
                let character = character.to_ascii_lowercase();

                if matches!(character, 'h' | '?') {
                    return None;
                }

                let drive_key = match style {
                    KeyboardDriveStyle::Wasd => matches!(character, 'w' | 'a' | 's' | 'd'),
                    KeyboardDriveStyle::Tank => matches!(character, 'w' | 's' | 'a' | 'd' | 'e'),
                };
                if drive_key {
                    return None;
                }

                match character {
                    'q' => Some(InputEvent::Command(BrainCommand::Quit)),
                    'p' => Some(InputEvent::Command(BrainCommand::Ping)),
                    'x' => Some(InputEvent::Command(BrainCommand::Stop)),
                    't' => Some(InputEvent::Command(BrainCommand::SetMode(Mode::Teleop))),
                    'u' => Some(InputEvent::Command(BrainCommand::SetMode(Mode::Autonomous))),
                    'f' => Some(InputEvent::Command(BrainCommand::SetMode(Mode::Fault))),
                    other => Some(InputEvent::Warning(InputWarning::UnknownKeyboardCommand(
                        other,
                    ))),
                }
            }
            _ => None,
        }
    }

    fn handle_key_event(&mut self, event: KeyEvent) -> Result<()> {
        let now = Instant::now();
        self.handle_key_event_at(event, now)
    }

    fn handle_key_event_at(&mut self, event: KeyEvent, now: Instant) -> Result<()> {
        if event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(event.code, KeyCode::Char('c') | KeyCode::Char('C'))
            && matches!(event.kind, KeyEventKind::Press)
        {
            self.enqueue_input_event(InputEvent::Command(BrainCommand::Quit));
            return Ok(());
        }

        if self.chat_buffer.is_some() {
            return self.handle_chat_key_event(event);
        }

        if matches!(event.kind, KeyEventKind::Press)
            && matches!(event.code, KeyCode::Char(CHAT_MODE_KEY) | KeyCode::Char('C'))
            && !event
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            self.enter_chat_mode(now);
            return Ok(());
        }

        if matches!(event.kind, KeyEventKind::Press)
            && matches!(
                event.code,
                KeyCode::Char(TOGGLE_DRIVE_STYLE_KEY) | KeyCode::Char('M')
            )
        {
            self.toggle_drive_style(now);
            return Ok(());
        }

        if matches!(
            event.kind,
            KeyEventKind::Press | KeyEventKind::Repeat | KeyEventKind::Release
        ) && HeldDriveKeys::is_drive_key(self.drive_style, event.code)
        {
            let control_changed = self.held_drive_keys.apply(
                self.drive_style,
                event.code,
                event.kind,
                now,
                self.keyboard_enhancement_enabled,
            );
            if let Some(lease) = drive_key_lease(event.kind) {
                // Any active drive-key activity should keep the combined desired direction alive,
                // even when the terminal only repeats one of the held keys.
                self.held_drive_keys.refresh_active(now, lease);
            }
            if control_changed {
                self.sync_control_state(now);
            }
        }

        if matches!(
            event.kind,
            KeyEventKind::Press | KeyEventKind::Repeat | KeyEventKind::Release
        ) && !HeldDriveKeys::is_drive_key(self.drive_style, event.code)
        {
            self.held_drive_keys
                .refresh_active(now, INTERLEAVED_KEY_LEASE);
        }

        if let Some(event) = Self::key_event_to_discrete_event(self.drive_style, event) {
            self.enqueue_input_event(event);
        }

        Ok(())
    }

    fn extend_active_control_at(&mut self, now: Instant, duration: Duration) {
        self.held_drive_keys.refresh_tracked(now, duration);
    }

    fn enqueue_input_event(&mut self, event: InputEvent) {
        self.pending_events.push_back(event);
    }

    fn sync_control_state(&mut self, now: Instant) {
        let control_state = self.held_drive_keys.to_control_state(self.drive_style, now);
        if control_state != self.current_control_state {
            self.current_control_state = control_state;
            self.enqueue_input_event(InputEvent::Control(control_state));
        }
    }

    fn expire_drive_keys_at(&mut self, now: Instant) {
        if self.held_drive_keys.expire(now) {
            self.sync_control_state(now);
        }
    }

    fn next_drive_key_expiration(&self, now: Instant) -> Option<Duration> {
        self.held_drive_keys.next_expiration(now)
    }

    fn poll_timeout(&self, timeout: Duration, now: Instant) -> Duration {
        self.next_drive_key_expiration(now)
            .map(|next_expiration| next_expiration.min(timeout))
            .unwrap_or(timeout)
    }

    fn handle_terminal_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key_event) => self.handle_key_event(key_event),
            Event::FocusLost => {
                self.held_drive_keys.reset();
                self.sync_control_state(Instant::now());
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn drain_ready_events(&mut self) -> Result<()> {
        while event::poll(Duration::ZERO)? {
            self.handle_terminal_event(event::read()?)?;
        }

        Ok(())
    }
}

impl CommandInputSource for KeyboardInput {
    fn instructions(&self) -> Option<Cow<'static, str>> {
        Some(Cow::Borrowed(self.help_text_for_current_style()))
    }

    fn next_event(&mut self) -> Result<InputEvent> {
        loop {
            if let Some(event) = self.poll_event(Duration::from_millis(250))? {
                return Ok(event);
            }
        }
    }

    fn poll_event(&mut self, timeout: Duration) -> Result<Option<InputEvent>> {
        self.ensure_raw_mode()?;

        if let Some(event) = self.pending_events.pop_front() {
            return Ok(Some(event));
        }

        self.expire_drive_keys_at(Instant::now());
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(Some(event));
        }

        let now = Instant::now();
        let poll_timeout = self.poll_timeout(timeout, now);
        if !event::poll(poll_timeout)? {
            self.expire_drive_keys_at(Instant::now());
            return Ok(self.pending_events.pop_front());
        }

        self.handle_terminal_event(event::read()?)?;
        self.drain_ready_events()?;
        self.expire_drive_keys_at(Instant::now());

        Ok(self.pending_events.pop_front())
    }

    fn suspend(&mut self) -> Result<()> {
        if self.raw_mode_enabled {
            self.drain_pending_events()?;
        }

        self.chat_buffer = None;
        self.held_drive_keys.reset();
        self.current_control_state = ControlState::default();
        self.pending_events.clear();
        self.disable_raw_mode_if_enabled()
    }

    fn resume(&mut self) -> Result<()> {
        self.ensure_raw_mode()?;
        self.chat_buffer = None;
        self.held_drive_keys.reset();
        self.current_control_state = ControlState::default();
        self.pending_events.clear();
        self.drain_pending_events()
    }

    fn extend_active_control(&mut self, duration: Duration) -> Result<()> {
        self.extend_active_control_at(Instant::now(), duration);
        Ok(())
    }
}

impl ControllerBackend for KeyboardInput {
    fn instructions(&self) -> Option<Cow<'static, str>> {
        Some(Cow::Borrowed(self.help_text_for_current_style()))
    }

    fn refresh_controllers(&mut self) -> Result<Vec<ControllerLifecycleEvent>> {
        if self.controller_connected {
            return Ok(Vec::new());
        }

        self.controller_connected = true;
        Ok(vec![ControllerLifecycleEvent::Connected(
            Self::controller_info(),
        )])
    }

    fn poll_input(&mut self, timeout: Duration) -> Result<Option<SourcedInputEvent>> {
        let Some(event) = CommandInputSource::poll_event(self, timeout)? else {
            return Ok(None);
        };

        let routed = match event {
            InputEvent::Command(command) => RoutedInputEvent::Command(command),
            InputEvent::Control(control_state) => RoutedInputEvent::Control(control_state),
            InputEvent::Warning(warning) => RoutedInputEvent::Warning(warning),
            InputEvent::Prompt(prompt) => RoutedInputEvent::Prompt(prompt),
            InputEvent::ControllerConnected(_) | InputEvent::ControllerDisconnected(_) => {
                return Ok(None);
            }
        };

        Ok(Some(SourcedInputEvent::new(
            Self::controller_info().id,
            routed,
        )))
    }

    fn suspend(&mut self) -> Result<()> {
        self.controller_connected = false;
        CommandInputSource::suspend(self)
    }

    fn resume(&mut self) -> Result<()> {
        self.controller_connected = false;
        CommandInputSource::resume(self)
    }

    fn extend_active_control(
        &mut self,
        controller: &ControllerId,
        duration: Duration,
    ) -> Result<()> {
        if *controller == Self::controller_info().id {
            CommandInputSource::extend_active_control(self, duration)?;
        }

        Ok(())
    }
}

impl Drop for KeyboardInput {
    fn drop(&mut self) {
        if self.raw_mode_enabled {
            let _ = self.disable_raw_mode_if_enabled();
            let _ = io::stdout().write_all(b"\r\n");
            let _ = io::stdout().flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::time::{Duration, Instant};

    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use mortimmy_core::Mode;

    use crate::{
        brain::BrainCommand,
        input::{ControlState, DriveIntent, InputEvent},
    };

    use super::{
        DRIVE_KEY_LEASE, INITIAL_DRIVE_KEY_LEASE, INTERLEAVED_KEY_LEASE, InputWarning,
        KeyboardDriveStyle, KeyboardInput,
    };

    fn drain_pending_events(input: &mut KeyboardInput) -> Vec<InputEvent> {
        input.pending_events.drain(..).collect()
    }

    fn enhanced_input() -> KeyboardInput {
        let mut input = KeyboardInput::new();
        input.keyboard_enhancement_enabled = true;
        input
    }

    fn tank_input() -> KeyboardInput {
        KeyboardInput::with_drive_style(KeyboardDriveStyle::Tank)
    }

    #[test]
    fn maps_single_key_events_without_enter() {
        let mut input = KeyboardInput::new();
        let now = Instant::now();

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Command(BrainCommand::Ping)]
        );

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Command(BrainCommand::SetMode(Mode::Teleop))]
        );
    }

    #[test]
    fn captures_chat_prompt_and_submits_chat_command() {
        let mut input = KeyboardInput::new();
        let now = Instant::now();

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![
                InputEvent::Prompt(Some("chat> ".to_string())),
                InputEvent::Warning(InputWarning::Status(Cow::Borrowed(
                    "chat mode active: type a message and press Enter",
                ))),
            ]
        );

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE), now)
            .unwrap();
        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![
                InputEvent::Prompt(Some("chat> h".to_string())),
                InputEvent::Prompt(Some("chat> hi".to_string())),
            ]
        );

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![
                InputEvent::Prompt(None),
                InputEvent::Command(BrainCommand::Chat("hi".to_string())),
            ]
        );
    }

    #[test]
    fn movement_keys_emit_drive_state_on_press_and_release() {
        let mut input = KeyboardInput::new();
        let now = Instant::now();

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            })]
        );

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: -DriveIntent::AXIS_MAX,
                    speed: 300,
                }),
            })]
        );

        let release_up = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::empty(),
        };
        input
            .handle_key_event_at(release_up, now + Duration::from_millis(1))
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: 0,
                    turn: -DriveIntent::AXIS_MAX,
                    speed: 300,
                }),
            })]
        );
    }

    #[test]
    fn allows_discrete_commands_while_drive_keys_are_held() {
        let mut input = KeyboardInput::new();
        let now = Instant::now();

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            })]
        );

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: -DriveIntent::AXIS_MAX,
                    speed: 300,
                }),
            })]
        );

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Command(BrainCommand::SetMode(Mode::Teleop))]
        );
    }

    #[test]
    fn drive_state_expires_and_can_be_triggered_again_without_release() {
        let mut input = KeyboardInput::new();
        let now = Instant::now();

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            })]
        );

        input.expire_drive_keys_at(now + DRIVE_KEY_LEASE + Duration::from_millis(1));
        assert!(drain_pending_events(&mut input).is_empty());

        input.expire_drive_keys_at(now + INITIAL_DRIVE_KEY_LEASE + Duration::from_millis(1));
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState { drive: None })]
        );

        input
            .handle_key_event_at(
                KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
                now + INITIAL_DRIVE_KEY_LEASE + Duration::from_millis(2),
            )
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            })]
        );
    }

    #[test]
    fn repeat_refresh_still_uses_short_lease_after_initial_press() {
        let mut input = KeyboardInput::new();
        let now = Instant::now();

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            })]
        );

        let repeat_up = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Repeat,
            state: KeyEventState::empty(),
        };
        input
            .handle_key_event_at(repeat_up, now + Duration::from_millis(100))
            .unwrap();
        assert!(drain_pending_events(&mut input).is_empty());

        input.expire_drive_keys_at(
            now + Duration::from_millis(100) + DRIVE_KEY_LEASE + Duration::from_millis(1),
        );
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState { drive: None })]
        );
    }

    #[test]
    fn second_drive_key_press_extends_existing_direction_hold() {
        let mut input = KeyboardInput::new();
        let now = Instant::now();

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            })]
        );

        input
            .handle_key_event_at(
                KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
                now + Duration::from_millis(400),
            )
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: DriveIntent::AXIS_MAX,
                    speed: 300,
                }),
            })]
        );

        input.expire_drive_keys_at(now + INITIAL_DRIVE_KEY_LEASE + Duration::from_millis(1));
        assert!(drain_pending_events(&mut input).is_empty());

        input.expire_drive_keys_at(
            now + Duration::from_millis(400) + INITIAL_DRIVE_KEY_LEASE + Duration::from_millis(1),
        );
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState { drive: None })]
        );
    }

    #[test]
    fn repeating_one_drive_key_keeps_combined_drive_hold_alive() {
        let mut input = KeyboardInput::new();
        let now = Instant::now();

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            })]
        );

        input
            .handle_key_event_at(
                KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
                now + Duration::from_millis(400),
            )
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: DriveIntent::AXIS_MAX,
                    speed: 300,
                }),
            })]
        );

        let repeat_right = KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Repeat,
            state: KeyEventState::empty(),
        };
        input
            .handle_key_event_at(repeat_right, now + Duration::from_millis(900))
            .unwrap();
        assert!(drain_pending_events(&mut input).is_empty());

        input.expire_drive_keys_at(now + Duration::from_millis(1_151));
        assert!(drain_pending_events(&mut input).is_empty());

        input.expire_drive_keys_at(
            now + Duration::from_millis(900) + DRIVE_KEY_LEASE + Duration::from_millis(1),
        );
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState { drive: None })]
        );
    }

    #[test]
    fn releasing_second_drive_key_preserves_first_direction_when_release_tracking_is_available() {
        let mut input = enhanced_input();
        let now = Instant::now();

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            })]
        );

        input
            .handle_key_event_at(
                KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
                now + Duration::from_millis(400),
            )
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: DriveIntent::AXIS_MAX,
                    speed: 300,
                }),
            })]
        );

        input.expire_drive_keys_at(now + Duration::from_millis(1_200));
        assert!(drain_pending_events(&mut input).is_empty());

        let release_right = KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::empty(),
        };
        input
            .handle_key_event_at(release_right, now + Duration::from_millis(1_200))
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            })]
        );
    }

    #[test]
    fn discrete_key_activity_extends_active_drive_hold() {
        let mut input = KeyboardInput::new();
        let now = Instant::now();

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            })]
        );

        let repeat_up = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Repeat,
            state: KeyEventState::empty(),
        };
        input
            .handle_key_event_at(repeat_up, now + Duration::from_millis(100))
            .unwrap();
        assert!(drain_pending_events(&mut input).is_empty());

        input
            .handle_key_event_at(
                KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
                now + Duration::from_millis(200),
            )
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Command(BrainCommand::Ping)]
        );

        input.expire_drive_keys_at(
            now + Duration::from_millis(200) + DRIVE_KEY_LEASE + Duration::from_millis(1),
        );
        assert!(drain_pending_events(&mut input).is_empty());

        input.expire_drive_keys_at(
            now + Duration::from_millis(200) + INTERLEAVED_KEY_LEASE + Duration::from_millis(1),
        );
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState { drive: None })]
        );
    }

    #[test]
    fn command_completion_can_rearm_tracked_drive_hold_after_deadline() {
        let mut input = KeyboardInput::new();
        let now = Instant::now();

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            })]
        );

        let command_done_at = now + INITIAL_DRIVE_KEY_LEASE + Duration::from_millis(50);
        input.extend_active_control_at(command_done_at, INITIAL_DRIVE_KEY_LEASE);

        input.expire_drive_keys_at(command_done_at + Duration::from_millis(300));
        assert!(drain_pending_events(&mut input).is_empty());

        input.expire_drive_keys_at(
            command_done_at + INITIAL_DRIVE_KEY_LEASE + Duration::from_millis(1),
        );
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState { drive: None })]
        );
    }

    #[test]
    fn ignores_release_events_for_discrete_commands() {
        let mut input = KeyboardInput::new();
        let event = KeyEvent {
            code: KeyCode::Char('p'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::empty(),
        };

        input.handle_key_event_at(event, Instant::now()).unwrap();
        assert!(drain_pending_events(&mut input).is_empty());
    }

    #[test]
    fn unknown_key_emits_warning_instead_of_error() {
        let mut input = KeyboardInput::new();

        input
            .handle_key_event_at(
                KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Warning(InputWarning::UnknownKeyboardCommand(
                'z'
            ))]
        );
    }

    #[test]
    fn tank_style_maps_tread_pairs_into_forward_and_turn() {
        let mut input = tank_input();
        let now = Instant::now();

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: 500,
                    turn: -500,
                    speed: 300,
                }),
            })]
        );

        input
            .handle_key_event_at(
                KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
                now + Duration::from_millis(1),
            )
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            })]
        );
    }

    #[test]
    fn tank_style_opposite_treads_pivot_in_place() {
        let mut input = tank_input();
        let now = Instant::now();

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE), now)
            .unwrap();
        assert!(!drain_pending_events(&mut input).is_empty());

        input
            .handle_key_event_at(
                KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
                now + Duration::from_millis(1),
            )
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: 0,
                    turn: -DriveIntent::AXIS_MAX,
                    speed: 300,
                }),
            })]
        );
    }

    #[test]
    fn tank_style_pivot_shortcut_matches_split_tread_input() {
        let mut input = tank_input();

        input
            .handle_key_event_at(
                KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: 0,
                    turn: -DriveIntent::AXIS_MAX,
                    speed: 300,
                }),
            })]
        );
    }

    #[test]
    fn toggle_drive_style_switches_key_mapping_and_stops_active_drive() {
        let mut input = KeyboardInput::new();
        let now = Instant::now();

        input
            .handle_key_event_at(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), now)
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            })]
        );

        input
            .handle_key_event_at(
                KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
                now + Duration::from_millis(1),
            )
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![
                InputEvent::Control(ControlState { drive: None }),
                InputEvent::Warning(InputWarning::Status(Cow::Borrowed(
                    "keyboard drive style switched to tank",
                ))),
            ]
        );

        input
            .handle_key_event_at(
                KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
                now + Duration::from_millis(2),
            )
            .unwrap();
        assert_eq!(
            drain_pending_events(&mut input),
            vec![InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: 500,
                    turn: DriveIntent::AXIS_MAX / 2,
                    speed: 300,
                }),
            })]
        );
    }
}
