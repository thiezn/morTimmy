use std::collections::{BTreeMap, VecDeque};

use mortimmy_core::Mode;

use crate::{
    config::LogLevel,
    input::{ControlState, ControllerId, ControllerInfo},
};

use super::completion::Suggestion;

pub const MAX_LOG_MESSAGES: usize = 200;

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
    pub command_input: String,
    pub cursor: usize,
    pub activity_scroll_offset: u16,
    pub logs: VecDeque<UiLogEntry>,
    pub show_help: bool,
    pub help_topic: Option<String>,
    pub completions: Vec<Suggestion>,
    pub selected_completion: usize,
}
