use std::collections::VecDeque;
use std::io::{Write, stdout};

use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute, queue,
    style::Print,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::{config::LogLevel, input::ControlState};

const MAX_LOG_MESSAGES: usize = 10;
const TITLE: &str = "mortimmy";

pub trait SessionOutput {
    fn log(&mut self, level: LogLevel, message: String) -> Result<()>;
    fn set_connection_status(&mut self, status: String) -> Result<()>;
    fn set_control_state(&mut self, control_state: ControlState) -> Result<()>;
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Default)]
pub struct NullSessionOutput;

impl SessionOutput for NullSessionOutput {
    fn log(&mut self, _level: LogLevel, _message: String) -> Result<()> {
        Ok(())
    }

    fn set_connection_status(&mut self, _status: String) -> Result<()> {
        Ok(())
    }

    fn set_control_state(&mut self, _control_state: ControlState) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UiLogEntry {
    level: LogLevel,
    message: String,
    repeats: usize,
}

pub struct SessionUi {
    log_level: LogLevel,
    no_color: bool,
    commands: Vec<String>,
    logs: VecDeque<UiLogEntry>,
    connection_status: String,
    control_state: ControlState,
    active: bool,
}

impl SessionUi {
    pub fn new(log_level: LogLevel, no_color: bool, commands: &str) -> Result<Self> {
        let mut ui = Self {
            log_level,
            no_color,
            commands: commands.lines().map(str::to_owned).collect(),
            logs: VecDeque::with_capacity(MAX_LOG_MESSAGES),
            connection_status: "Connecting".to_string(),
            control_state: ControlState::default(),
            active: false,
        };

        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, Hide, Clear(ClearType::All), MoveTo(0, 0))?;
        ui.active = true;
        ui.render()?;
        Ok(ui)
    }

    fn allows(&self, level: LogLevel) -> bool {
        severity(level) >= severity(self.log_level)
    }

    fn render(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }

        let (width, height) = terminal::size().unwrap_or((120, 30));
        let width = usize::from(width.max(20));
        let height = usize::from(height.max(10));

        let mut lines = vec![
            TITLE.to_string(),
            format!("Connection: {}", self.connection_status),
            format!("Drive: {}", describe_control_state(self.control_state)),
            String::new(),
        ];
        lines.extend(self.commands.iter().cloned());
        lines.push(String::new());
        lines.push("Recent messages:".to_string());

        let fixed_lines = lines.len();
        let available_log_lines = height.saturating_sub(fixed_lines);
        lines.extend(rendered_log_lines(&self.logs, width, available_log_lines));

        let mut stdout = stdout();
        execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
        for (row, line) in lines.into_iter().take(height).enumerate() {
            queue!(
                stdout,
                MoveTo(0, row as u16),
                Print(fit_line(&line, width, self.no_color))
            )?;
        }
        stdout.flush()?;
        Ok(())
    }
}

impl SessionOutput for SessionUi {
    fn log(&mut self, level: LogLevel, message: String) -> Result<()> {
        if !self.allows(level) {
            return Ok(());
        }

        let message = sanitize_message(message);
        if let Some(last) = self.logs.back_mut()
            && last.level == level
            && last.message == message
        {
            last.repeats = last.repeats.saturating_add(1);
            return self.render();
        }

        if self.logs.len() == MAX_LOG_MESSAGES {
            self.logs.pop_front();
        }
        self.logs.push_back(UiLogEntry {
            level,
            message,
            repeats: 1,
        });
        self.render()
    }

    fn set_connection_status(&mut self, status: String) -> Result<()> {
        self.connection_status = status;
        self.render()
    }

    fn set_control_state(&mut self, control_state: ControlState) -> Result<()> {
        self.control_state = control_state;
        self.render()
    }
}

impl Drop for SessionUi {
    fn drop(&mut self) {
        if self.active {
            let mut stdout = stdout();
            let _ = execute!(stdout, Show, LeaveAlternateScreen);
            let _ = stdout.flush();
        }
    }
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

fn describe_control_state(control_state: ControlState) -> String {
    match control_state.drive {
        Some(drive) => format!(
            "forward={} turn={} speed={}",
            drive.forward, drive.turn, drive.speed
        ),
        None => "idle".to_string(),
    }
}

fn format_log_entry(entry: &UiLogEntry) -> String {
    let base = format!("[{}] {}", level_label(entry.level), entry.message);
    if entry.repeats > 1 {
        format!("{} (x{})", base, entry.repeats)
    } else {
        base
    }
}

fn rendered_log_lines(logs: &VecDeque<UiLogEntry>, width: usize, available_lines: usize) -> Vec<String> {
    if width == 0 || available_lines == 0 {
        return Vec::new();
    }

    let mut rendered = VecDeque::new();
    let mut used_lines = 0;

    for entry in logs.iter().rev() {
        let entry_lines = wrap_line(&format_log_entry(entry), width);
        if entry_lines.len() > available_lines {
            if rendered.is_empty() {
                return entry_lines.into_iter().take(available_lines).collect();
            }
            break;
        }

        if used_lines + entry_lines.len() > available_lines {
            break;
        }

        used_lines += entry_lines.len();
        for line in entry_lines.into_iter().rev() {
            rendered.push_front(line);
        }
    }

    rendered.into_iter().collect()
}

fn wrap_line(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }

    let characters: Vec<char> = line.chars().collect();
    if characters.is_empty() {
        return vec![String::new()];
    }

    characters
        .chunks(width)
        .map(|chunk| chunk.iter().collect())
        .collect()
}

fn level_label(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Trace => "TRACE",
        LogLevel::Debug => "DEBUG",
        LogLevel::Info => "INFO",
        LogLevel::Warn => "WARN",
        LogLevel::Error => "ERROR",
    }
}

fn fit_line(line: &str, width: usize, _no_color: bool) -> String {
    let char_count = line.chars().count();
    if char_count <= width {
        return line.to_string();
    }
    if width <= 3 {
        return "...".to_string();
    }

    let mut result = String::new();
    for character in line.chars().take(width - 3) {
        result.push(character);
    }
    result.push_str("...");
    result
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use crate::config::LogLevel;

    use super::{UiLogEntry, format_log_entry, rendered_log_lines, wrap_line};

    #[test]
    fn wraps_long_log_entries_over_multiple_lines() {
        let entry = UiLogEntry {
            level: LogLevel::Info,
            message: "telemetry desired-state: DesiredState(DesiredStateTelemetry { mode: Teleop, drive: MotorStateTelemetry { left: PwmTicks(300), right: PwmTicks(0) }, servo: ServoStateTelemetry { pan: ServoTicks(0), tilt: ServoTicks(0) }, error: None })".to_string(),
            repeats: 1,
        };

        let wrapped = wrap_line(&format_log_entry(&entry), 32);

        assert!(wrapped.len() > 1);
        assert!(wrapped.iter().all(|line| line.chars().count() <= 32));
        assert_eq!(wrapped.concat(), format_log_entry(&entry));
    }

    #[test]
    fn rendered_log_lines_keep_newest_complete_entries() {
        let logs = VecDeque::from([
            UiLogEntry {
                level: LogLevel::Info,
                message: "older entry".to_string(),
                repeats: 1,
            },
            UiLogEntry {
                level: LogLevel::Warn,
                message: "newest entry that wraps across multiple display rows".to_string(),
                repeats: 1,
            },
        ]);

        let rendered = rendered_log_lines(&logs, 20, 3);
        let newest_entry = format_log_entry(logs.back().unwrap());
        let expected_prefix: String = newest_entry.chars().take(60).collect();

        assert_eq!(rendered.len(), 3);
        assert!(rendered.iter().all(|line| line.chars().count() <= 20));
        assert!(!rendered.iter().any(|line| line.contains("older entry")));
        assert_eq!(rendered.concat(), expected_prefix);
    }
}
