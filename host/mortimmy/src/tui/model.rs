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
    w_down: bool,
    a_down: bool,
    s_down: bool,
    d_down: bool,
    e_down: bool,
    space_down: bool,
}

impl Default for KeyboardDriveState {
    fn default() -> Self {
        Self {
            style: KeyboardDriveStyle::Arcade,
            forward_axis: 0,
            turn_axis: 0,
            left_track: 0,
            right_track: 0,
            w_down: false,
            a_down: false,
            s_down: false,
            d_down: false,
            e_down: false,
            space_down: false,
        }
    }
}

impl KeyboardDriveState {
    pub fn apply_key_down(&mut self, key: char) -> bool {
        self.apply_key_state(key, true)
    }

    pub fn apply_key_up(&mut self, key: char) -> bool {
        self.apply_key_state(key, false)
    }

    fn apply_key_state(&mut self, key: char, is_down: bool) -> bool {
        match self.style {
            KeyboardDriveStyle::Arcade => self.apply_arcade_key_state(key, is_down),
            KeyboardDriveStyle::Tank => self.apply_tank_key_state(key, is_down),
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
        self.w_down = false;
        self.a_down = false;
        self.s_down = false;
        self.d_down = false;
        self.e_down = false;
        self.space_down = false;
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

    fn apply_arcade_key_state(&mut self, key: char, is_down: bool) -> bool {
        match key {
            'w' => self.w_down = is_down,
            'a' => self.a_down = is_down,
            's' => self.s_down = is_down,
            'd' => self.d_down = is_down,
            ' ' => self.space_down = is_down,
            _ => return false,
        }

        let previous = (self.forward_axis, self.turn_axis, self.left_track, self.right_track);

        if self.space_down {
            self.forward_axis = 0;
            self.turn_axis = 0;
        } else {
            self.forward_axis = axis_from_pair(self.w_down, self.s_down);
            self.turn_axis = axis_from_pair(self.d_down, self.a_down);
        }
        self.left_track = 0;
        self.right_track = 0;

        previous != (self.forward_axis, self.turn_axis, self.left_track, self.right_track)
    }

    fn apply_tank_key_state(&mut self, key: char, is_down: bool) -> bool {
        match key {
            'w' => self.w_down = is_down,
            's' => self.s_down = is_down,
            'e' => self.e_down = is_down,
            'd' => self.d_down = is_down,
            ' ' => self.space_down = is_down,
            _ => return false,
        }

        let previous = (self.forward_axis, self.turn_axis, self.left_track, self.right_track);

        let (left_track, right_track) = if self.space_down {
            (0, 0)
        } else {
            (
                axis_from_pair(self.w_down, self.s_down),
                axis_from_pair(self.e_down, self.d_down),
            )
        };

        self.forward_axis = 0;
        self.turn_axis = 0;
        self.left_track = left_track;
        self.right_track = right_track;

        previous != (self.forward_axis, self.turn_axis, self.left_track, self.right_track)
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

const fn axis_from_pair(positive: bool, negative: bool) -> i8 {
    if positive == negative {
        0
    } else if positive {
        1
    } else {
        -1
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
        assert!(state.apply_key_down('w'));
        assert_eq!(state.control_state().drive.unwrap().forward, 1_000);
        assert_eq!(state.control_state().drive.unwrap().turn, 0);

        assert!(state.apply_key_down('a'));
        assert_eq!(state.control_state().drive.unwrap().forward, 1_000);
        assert_eq!(state.control_state().drive.unwrap().turn, -1_000);

        assert!(state.apply_key_up('a'));
        assert_eq!(state.control_state().drive.unwrap().forward, 1_000);
        assert_eq!(state.control_state().drive.unwrap().turn, 0);

        assert!(state.apply_key_up('w'));
        assert_eq!(state.control_state().drive, None);

        assert!(!state.apply_key_down(' '));
        assert_eq!(state.control_state().drive, None);
    }

    #[test]
    fn tank_keyboard_drive_combines_left_and_right_tracks() {
        let mut state = KeyboardDriveState {
            style: KeyboardDriveStyle::Tank,
            ..KeyboardDriveState::default()
        };

        assert!(state.apply_key_down('w'));
        assert_eq!(state.control_state().drive.unwrap().forward, 500);
        assert_eq!(state.control_state().drive.unwrap().turn, -500);

        assert!(state.apply_key_down('e'));
        assert_eq!(state.control_state().drive.unwrap().forward, 1_000);
        assert_eq!(state.control_state().drive.unwrap().turn, 0);

        assert!(state.apply_key_down('d'));
        assert_eq!(state.control_state().drive.unwrap().forward, 500);
        assert_eq!(state.control_state().drive.unwrap().turn, -500);

        assert!(state.apply_key_up('d'));
        assert_eq!(state.control_state().drive.unwrap().forward, 1_000);
        assert_eq!(state.control_state().drive.unwrap().turn, 0);
    }

    #[test]
    fn toggling_keyboard_drive_style_resets_motion() {
        let mut state = KeyboardDriveState::default();
        state.apply_key_down('w');

        state.toggle_style();

        assert_eq!(state.style, KeyboardDriveStyle::Tank);
        assert_eq!(state.control_state().drive, None);
    }
}
