use std::collections::{BTreeMap, VecDeque};

use mortimmy_core::Mode;
use mortimmy_protocol::messages::telemetry::RangeTelemetry;

use crate::{
    config::LogLevel,
    input::{ControlState, ControllerId, ControllerInfo, DriveIntent},
};

use super::completion::Suggestion;

pub const MAX_LOG_MESSAGES: usize = 200;
pub const KEYBOARD_DRIVE_SPEED: u16 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyboardDriveStyle {
    #[default]
    Arcade,
    Tank,
}

impl KeyboardDriveStyle {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Arcade => "w,a,s,d",
            Self::Tank => "tank",
        }
    }

    pub const fn toggled(self) -> Self {
        match self {
            Self::Arcade => Self::Tank,
            Self::Tank => Self::Arcade,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardDriveState {
    pub style: KeyboardDriveStyle,
    forward_axis: i8,
    turn_axis: i8,
    left_track: i8,
    right_track: i8,
}

impl Default for KeyboardDriveState {
    fn default() -> Self {
        Self {
            style: KeyboardDriveStyle::Arcade,
            forward_axis: 0,
            turn_axis: 0,
            left_track: 0,
            right_track: 0,
        }
    }
}

impl KeyboardDriveState {
    pub fn apply_key(&mut self, key: char) -> bool {
        match self.style {
            KeyboardDriveStyle::Arcade => self.apply_arcade_key(key),
            KeyboardDriveStyle::Tank => self.apply_tank_key(key),
        }
    }

    pub fn toggle_style(&mut self) {
        self.style = self.style.toggled();
        self.reset_motion();
    }

    pub fn reset_motion(&mut self) {
        self.forward_axis = 0;
        self.turn_axis = 0;
        self.left_track = 0;
        self.right_track = 0;
    }

    pub fn control_state(self) -> ControlState {
        let (forward, turn) = match self.style {
            KeyboardDriveStyle::Arcade => (
                axis_to_intent(self.forward_axis),
                axis_to_intent(self.turn_axis),
            ),
            KeyboardDriveStyle::Tank => {
                let left = i32::from(axis_to_intent(self.left_track));
                let right = i32::from(axis_to_intent(self.right_track));
                (((left + right) / 2) as i16, ((right - left) / 2) as i16)
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

    fn apply_arcade_key(&mut self, key: char) -> bool {
        let next = match key {
            'w' => (1, 0),
            'a' => (0, -1),
            's' => (-1, 0),
            'd' => (0, 1),
            ' ' => (0, 0),
            _ => return false,
        };

        let changed = (self.forward_axis, self.turn_axis) != next || self.left_track != 0 || self.right_track != 0;
        self.forward_axis = next.0;
        self.turn_axis = next.1;
        self.left_track = 0;
        self.right_track = 0;
        changed
    }

    fn apply_tank_key(&mut self, key: char) -> bool {
        let (left_track, right_track) = match key {
            'w' => (1, self.right_track),
            's' => (-1, self.right_track),
            'e' => (self.left_track, 1),
            'd' => (self.left_track, -1),
            ' ' => (0, 0),
            _ => return false,
        };

        let changed = self.left_track != left_track || self.right_track != right_track || self.forward_axis != 0 || self.turn_axis != 0;
        self.forward_axis = 0;
        self.turn_axis = 0;
        self.left_track = left_track;
        self.right_track = right_track;
        changed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Command,
    KeyboardDrive(KeyboardDriveState),
}

impl InputMode {
    pub const fn keyboard_drive(self) -> Option<KeyboardDriveState> {
        match self {
            Self::KeyboardDrive(state) => Some(state),
            Self::Command => None,
        }
    }
}

const fn axis_to_intent(value: i8) -> i16 {
    match value {
        -1 => -DriveIntent::AXIS_MAX,
        1 => DriveIntent::AXIS_MAX,
        _ => 0,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiLogEntry {
    pub level: LogLevel,
    pub message: String,
    pub repeats: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryStatus {
    pub config_path: String,
    pub connection_status: String,
    pub control_state: ControlState,
    pub desired_mode: Mode,
    pub distance: Option<RangeTelemetry>,
    pub transport_label: String,
    pub serial_target: String,
    pub nexo_gateway: String,
    pub nexo_client: String,
    pub controller_selection: String,
    pub active_controllers: BTreeMap<ControllerId, ControllerInfo>,
}

impl Default for SummaryStatus {
    fn default() -> Self {
        Self {
            config_path: String::new(),
            connection_status: "connecting".to_string(),
            control_state: ControlState::default(),
            desired_mode: Mode::Teleop,
            distance: None,
            transport_label: String::new(),
            serial_target: String::new(),
            nexo_gateway: String::new(),
            nexo_client: String::new(),
            controller_selection: String::new(),
            active_controllers: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct Model {
    pub log_level: LogLevel,
    pub no_color: bool,
    pub summary: SummaryStatus,
    pub input_mode: InputMode,
    pub command_input: String,
    pub cursor: usize,
    pub activity_scroll_offset: u16,
    pub logs: VecDeque<UiLogEntry>,
    pub show_help: bool,
    pub help_topic: Option<String>,
    pub completions: Vec<Suggestion>,
    pub selected_completion: usize,
}

#[cfg(test)]
mod tests {
    use super::{KeyboardDriveState, KeyboardDriveStyle};

    #[test]
    fn arcade_keyboard_drive_maps_wasd_to_control_state() {
        let mut state = KeyboardDriveState::default();
        assert!(state.apply_key('w'));
        assert_eq!(state.control_state().drive.unwrap().forward, 1_000);
        assert_eq!(state.control_state().drive.unwrap().turn, 0);

        assert!(state.apply_key('a'));
        assert_eq!(state.control_state().drive.unwrap().forward, 0);
        assert_eq!(state.control_state().drive.unwrap().turn, -1_000);

        assert!(state.apply_key(' '));
        assert_eq!(state.control_state().drive, None);
    }

    #[test]
    fn tank_keyboard_drive_combines_left_and_right_tracks() {
        let mut state = KeyboardDriveState {
            style: KeyboardDriveStyle::Tank,
            ..KeyboardDriveState::default()
        };

        assert!(state.apply_key('w'));
        assert_eq!(state.control_state().drive.unwrap().forward, 500);
        assert_eq!(state.control_state().drive.unwrap().turn, -500);

        assert!(state.apply_key('e'));
        assert_eq!(state.control_state().drive.unwrap().forward, 1_000);
        assert_eq!(state.control_state().drive.unwrap().turn, 0);

        assert!(state.apply_key('d'));
        assert_eq!(state.control_state().drive.unwrap().forward, 0);
        assert_eq!(state.control_state().drive.unwrap().turn, -1_000);
    }

    #[test]
    fn toggling_keyboard_drive_style_resets_motion() {
        let mut state = KeyboardDriveState::default();
        state.apply_key('w');

        state.toggle_style();

        assert_eq!(state.style, KeyboardDriveStyle::Tank);
        assert_eq!(state.control_state().drive, None);
    }
}
