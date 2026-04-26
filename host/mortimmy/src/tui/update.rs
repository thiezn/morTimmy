use super::{message::Message, model::{MAX_LOG_MESSAGES, Model, UiLogEntry}};
use crate::config::LogLevel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    ExecuteCommand(String),
    ApplySelectedCompletion,
    CopyAllActivity,
    CopyLastActivity,
}

pub fn update(model: &mut Model, msg: Message) -> Action {
    match msg {
        Message::InsertChar(character) => {
            model.command_input.insert(model.cursor, character);
            model.cursor += character.len_utf8();
        }
        Message::Backspace => {
            if model.cursor > 0 {
                model.cursor -= 1;
                model.command_input.remove(model.cursor);
            }
        }
        Message::DeleteForward => {
            if model.cursor < model.command_input.len() {
                model.command_input.remove(model.cursor);
            }
        }
        Message::MoveCursorLeft => {
            model.cursor = model.cursor.saturating_sub(1);
        }
        Message::MoveCursorRight => {
            model.cursor = (model.cursor + 1).min(model.command_input.len());
        }
        Message::MoveCursorStart => {
            model.cursor = 0;
        }
        Message::MoveCursorEnd => {
            model.cursor = model.command_input.len();
        }
        Message::SubmitInput => {
            let command = model.command_input.trim().to_string();
            model.command_input.clear();
            model.cursor = 0;
            model.completions.clear();
            model.selected_completion = 0;
            if !command.is_empty() {
                model.show_help = false;
                model.help_topic = None;
                return Action::ExecuteCommand(command);
            }
        }
        Message::SelectNextCompletion => {
            if !model.completions.is_empty() {
                model.selected_completion = (model.selected_completion + 1) % model.completions.len();
            }
        }
        Message::SelectPreviousCompletion => {
            if !model.completions.is_empty() {
                model.selected_completion = if model.selected_completion == 0 {
                    model.completions.len() - 1
                } else {
                    model.selected_completion - 1
                };
            }
        }
        Message::ApplySelectedCompletion => {
            if !model.completions.is_empty() {
                return Action::ApplySelectedCompletion;
            }
        }
        Message::ScrollActivityUp => {
            model.activity_scroll_offset = model.activity_scroll_offset.saturating_add(3);
        }
        Message::ScrollActivityDown => {
            model.activity_scroll_offset = model.activity_scroll_offset.saturating_sub(3);
        }
        Message::CopyAllActivity => {
            return Action::CopyAllActivity;
        }
        Message::CopyLastActivity => {
            return Action::CopyLastActivity;
        }
        Message::SetConnectionStatus(status) => {
            model.summary.connection_status = status;
        }
        Message::SetControlState(control_state) => {
            model.summary.control_state = control_state;
        }
        Message::SetDesiredMode(mode) => {
            model.summary.desired_mode = mode;
        }
        Message::SetDistance(distance) => {
            model.summary.distance = distance;
        }
        Message::Log(level, message) => {
            if allows(model.log_level, level) {
                let message = sanitize_message(message);
                if let Some(last) = model.logs.back_mut()
                    && last.level == level
                    && last.message == message
                {
                    last.repeats = last.repeats.saturating_add(1);
                    return Action::None;
                }

                if model.logs.len() == MAX_LOG_MESSAGES {
                    model.logs.pop_front();
                }
                model.logs.push_back(UiLogEntry {
                    level,
                    message,
                    repeats: 1,
                });
            }
        }
        Message::ShowHelp(visible) => {
            model.show_help = visible;
            if !visible {
                model.help_topic = None;
            }
        }
        Message::ControllerConnected(controller) => {
            model.summary.active_controllers.insert(controller.id.clone(), controller);
        }
        Message::ControllerDisconnected(controller) => {
            model.summary.active_controllers.remove(&controller.id);
        }
    }

    Action::None
}

fn allows(current: LogLevel, incoming: LogLevel) -> bool {
    severity(incoming) >= severity(current)
}

fn severity(level: LogLevel) -> u8 {
    match level {
        LogLevel::Trace => 0,
        LogLevel::Debug => 1,
        LogLevel::Info => 2,
        LogLevel::Warn => 3,
        LogLevel::Error => 4,
    }
}

fn sanitize_message(message: String) -> String {
    message
        .replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
