use anyhow::{Result, bail};
use mortimmy_core::Mode;

use crate::{
    brain::BrainCommand,
    input::{ControlState, DriveIntent, InputEvent},
};

use super::files::FileIndex;

const DEFAULT_DRIVE_SPEED: u16 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    pub name: &'static str,
    pub usage: &'static str,
    pub summary: &'static str,
    pub details: &'static [&'static str],
}

pub const COMMAND_SPECS: &[CommandSpec] = &[
    CommandSpec {
        name: "help",
        usage: "/help [command]",
        summary: "Show the full command reference.",
        details: &[
            "Use `/help` to open the general command reference.",
            "Use `/help <command>` to show detailed help for a single command.",
            "Examples: `/help mode`, `/help drive`.",
        ],
    },
    CommandSpec {
        name: "quit",
        usage: "/quit",
        summary: "Exit the brain loop.",
        details: &[
            "Exit the mortimmy TUI and stop the brain loop.",
            "Keyboard shortcut: Ctrl+X or Ctrl+C.",
        ],
    },
    CommandSpec {
        name: "ping",
        usage: "/ping",
        summary: "Send a ping and wait for pong telemetry.",
        details: &[
            "Send a ping to the transport and wait for the matching pong telemetry.",
            "Useful when checking whether the controller link is alive.",
        ],
    },
    CommandSpec {
        name: "stop",
        usage: "/stop",
        summary: "Stop all motion and reset to the default mode.",
        details: &[
            "Stop drive motion, clear active control, and reset the desired mode to the default teleop state.",
            "Useful as an immediate soft stop while staying in the TUI session.",
        ],
    },
    CommandSpec {
        name: "mode",
        usage: "/mode <teleop|autonomous|fault>",
        summary: "Switch the robot operating mode.",
        details: &[
            "`teleop` enables manual drive control.",
            "`autonomous` hands motion over to the autonomy runner.",
            "`fault` commands a safe stopped state.",
            "Examples: `/mode teleop`, `/mode autonomous`, `/mode fault`.",
        ],
    },
    CommandSpec {
        name: "servo",
        usage: "/servo <pan> <tilt>",
        summary: "Move the pan and tilt servos to raw tick values.",
        details: &[
            "Set the raw pan and tilt servo positions in ticks.",
            "Use this when you want explicit camera/head positioning rather than drive motion.",
            "Example: `/servo 1200 900`.",
        ],
    },
    CommandSpec {
        name: "drive",
        usage: "/drive <forward|backward|left|right|stop|<forward> <turn> [speed]>",
        summary: "Set local teleop drive intent with direction keywords or raw axes.",
        details: &[
            "Directional shortcuts: `forward`, `backward`, `left`, `right`, `stop`.",
            "Raw axis form: `/drive <forward> <turn> [speed]` where forward and turn are between -32767 and 32767.",
            "Examples: `/drive forward`, `/drive right 450`, `/drive 12000 -8000 300`.",
        ],
    },
    CommandSpec {
        name: "chat",
        usage: "/chat <prompt with optional @file references>",
        summary: "Send a chat request through nexo. @file expands file contents into the prompt.",
        details: &[
            "Send a prompt to the nexo gateway in the background; replies appear in Activity when they arrive.",
            "Bare input without `/` is treated as `/chat <prompt>`.",
            "Use `@filename` to inline workspace file contents into the prompt.",
            "Example: `/chat summarize @README.md`.",
        ],
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalCommand {
    Help(Option<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAction {
    Emit(InputEvent),
    Local(LocalCommand),
}

pub fn parse(input: &str, files: &FileIndex) -> Result<CommandAction> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!("command input is empty");
    }
    if !trimmed.starts_with('/') {
        return parse_chat(trimmed, files);
    }

    let body = trimmed.trim_start_matches('/');
    let (command_name, rest) = body
        .split_once(char::is_whitespace)
        .map(|(name, rest)| (name, rest.trim()))
        .unwrap_or((body, ""));

    match command_name.to_ascii_lowercase().as_str() {
        "help" => parse_help(rest),
        "quit" => Ok(CommandAction::Emit(InputEvent::Command(BrainCommand::Quit))),
        "ping" => Ok(CommandAction::Emit(InputEvent::Command(BrainCommand::Ping))),
        "stop" => Ok(CommandAction::Emit(InputEvent::Command(BrainCommand::Stop))),
        "mode" => parse_mode(rest),
        "servo" => parse_servo(rest),
        "drive" => parse_drive(rest),
        "chat" => parse_chat(rest, files),
        other => bail!("unknown command `/{other}`; try /help"),
    }
}

pub fn help_text(topic: Option<&str>) -> String {
    if let Some(topic) = topic {
        if let Some(spec) = command_spec(topic) {
            return detailed_help_text(spec);
        }
    }

    let mut lines = vec!["Available commands:".to_string()];
    for spec in COMMAND_SPECS {
        lines.push(format!("  {:<16} {}", spec.usage, spec.summary));
    }
    lines.push(String::new());
    lines.push("Notes:".to_string());
    lines.push("  Commands must start with /.".to_string());
    lines.push("  Bare input is treated as /chat <prompt>.".to_string());
    lines.push("  Use @filename inside /chat to inline file contents into the prompt.".to_string());
    lines.push("  Press Tab to autocomplete commands, subcommands, and @file references.".to_string());
    lines.push("  Use /help <command> for detailed help on one command.".to_string());
    lines.join("\n")
}

pub fn command_spec(name: &str) -> Option<&'static CommandSpec> {
    COMMAND_SPECS
        .iter()
        .find(|spec| spec.name.eq_ignore_ascii_case(name.trim_start_matches('/')))
}

pub fn mode_names() -> &'static [&'static str] {
    &["teleop", "autonomous", "fault"]
}

pub fn drive_keywords() -> &'static [&'static str] {
    &["forward", "backward", "left", "right", "stop"]
}

fn parse_help(rest: &str) -> Result<CommandAction> {
    let trimmed = rest.trim();
    if trimmed.is_empty() {
        return Ok(CommandAction::Local(LocalCommand::Help(None)));
    }

    let mut parts = trimmed.split_whitespace();
    let topic = parts.next().unwrap_or_default();
    if parts.next().is_some() {
        bail!("usage: /help [command]");
    }

    let spec = command_spec(topic)
        .ok_or_else(|| anyhow::anyhow!("unknown command `{topic}`; try /help"))?;
    Ok(CommandAction::Local(LocalCommand::Help(Some(spec.name.to_string()))))
}

fn parse_mode(rest: &str) -> Result<CommandAction> {
    let mode = match rest {
        "teleop" => Mode::Teleop,
        "autonomous" => Mode::Autonomous,
        "fault" => Mode::Fault,
        _ => bail!("usage: /mode <teleop|autonomous|fault>"),
    };

    Ok(CommandAction::Emit(InputEvent::Command(BrainCommand::SetMode(mode))))
}

fn parse_servo(rest: &str) -> Result<CommandAction> {
    let mut parts = rest.split_whitespace();
    let pan: u16 = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: /servo <pan> <tilt>"))?
        .parse()?;
    let tilt: u16 = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: /servo <pan> <tilt>"))?
        .parse()?;
    if parts.next().is_some() {
        bail!("usage: /servo <pan> <tilt>");
    }

    Ok(CommandAction::Emit(InputEvent::Command(BrainCommand::Servo {
        pan,
        tilt,
    })))
}

fn parse_drive(rest: &str) -> Result<CommandAction> {
    let parts: Vec<_> = rest.split_whitespace().collect();
    if parts.is_empty() {
        bail!("usage: /drive <forward|backward|left|right|stop|<forward> <turn> [speed]>");
    }

    let control = match parts.as_slice() {
        ["stop"] => ControlState::default(),
        ["forward"] => directional_drive(DriveIntent::AXIS_MAX, 0, DEFAULT_DRIVE_SPEED),
        ["backward"] => directional_drive(-DriveIntent::AXIS_MAX, 0, DEFAULT_DRIVE_SPEED),
        ["left"] => directional_drive(0, -DriveIntent::AXIS_MAX, DEFAULT_DRIVE_SPEED),
        ["right"] => directional_drive(0, DriveIntent::AXIS_MAX, DEFAULT_DRIVE_SPEED),
        [direction, speed] if *direction == "forward" => {
            directional_drive(DriveIntent::AXIS_MAX, 0, speed.parse()?)
        }
        [direction, speed] if *direction == "backward" => {
            directional_drive(-DriveIntent::AXIS_MAX, 0, speed.parse()?)
        }
        [direction, speed] if *direction == "left" => {
            directional_drive(0, -DriveIntent::AXIS_MAX, speed.parse()?)
        }
        [direction, speed] if *direction == "right" => {
            directional_drive(0, DriveIntent::AXIS_MAX, speed.parse()?)
        }
        [forward, turn] => directional_drive(parse_axis(forward)?, parse_axis(turn)?, DEFAULT_DRIVE_SPEED),
        [forward, turn, speed] => directional_drive(parse_axis(forward)?, parse_axis(turn)?, speed.parse()?),
        _ => bail!("usage: /drive <forward|backward|left|right|stop|<forward> <turn> [speed]>"),
    };

    Ok(CommandAction::Emit(InputEvent::Control(control)))
}

fn parse_chat(rest: &str, files: &FileIndex) -> Result<CommandAction> {
    if rest.is_empty() {
        bail!("usage: /chat <prompt>");
    }

    let expanded = files.expand_references(rest)?;
    Ok(CommandAction::Emit(InputEvent::Command(BrainCommand::Chat(
        expanded.text,
    ))))
}

fn parse_axis(value: &str) -> Result<i16> {
    let axis: i16 = value.parse()?;
    if !(-DriveIntent::AXIS_MAX..=DriveIntent::AXIS_MAX).contains(&axis) {
        bail!(
            "drive axis must be between {} and {}",
            -DriveIntent::AXIS_MAX,
            DriveIntent::AXIS_MAX
        );
    }
    Ok(axis)
}

fn directional_drive(forward: i16, turn: i16, speed: u16) -> ControlState {
    let drive = if forward == 0 && turn == 0 {
        None
    } else {
        Some(DriveIntent {
            forward,
            turn,
            speed,
        })
    };
    ControlState { drive }
}

fn detailed_help_text(spec: &CommandSpec) -> String {
    let mut lines = vec![format!("Command: /{}", spec.name), String::new()];
    lines.push(format!("Usage: {}", spec.usage));
    lines.push(format!("Summary: {}", spec.summary));
    if !spec.details.is_empty() {
        lines.push(String::new());
        lines.push("Details:".to_string());
        for detail in spec.details {
            lines.push(format!("  {}", detail));
        }
    }
    lines
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use mortimmy_core::Mode;

    use super::{CommandAction, LocalCommand, help_text, parse};
    use crate::{
        brain::BrainCommand,
        input::{ControlState, DriveIntent, InputEvent},
        tui::files::FileIndex,
    };

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos))
    }

    #[test]
    fn parses_help_into_local_action() {
        assert_eq!(
            parse("/help", &FileIndex::default()).unwrap(),
            CommandAction::Local(LocalCommand::Help(None))
        );
        assert_eq!(
            parse("/help mode", &FileIndex::default()).unwrap(),
            CommandAction::Local(LocalCommand::Help(Some("mode".to_string())))
        );
        match parse("hello mortimmy", &FileIndex::default()).unwrap() {
            CommandAction::Emit(InputEvent::Command(BrainCommand::Chat(prompt))) => {
                assert_eq!(prompt, "hello mortimmy");
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
        assert!(help_text(None).contains("/chat <prompt with optional @file references>"));
        assert!(help_text(Some("mode")).contains("Command: /mode"));
        assert!(help_text(Some("mode")).contains("/mode <teleop|autonomous|fault>"));
    }

    #[test]
    fn parses_mode_drive_and_servo_commands() {
        assert_eq!(
            parse("/mode teleop", &FileIndex::default()).unwrap(),
            CommandAction::Emit(InputEvent::Command(BrainCommand::SetMode(Mode::Teleop)))
        );
        assert_eq!(
            parse("/servo 12 18", &FileIndex::default()).unwrap(),
            CommandAction::Emit(InputEvent::Command(BrainCommand::Servo { pan: 12, tilt: 18 }))
        );
        assert_eq!(
            parse("/drive forward 450", &FileIndex::default()).unwrap(),
            CommandAction::Emit(InputEvent::Control(ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 450,
                }),
            }))
        );
    }

    #[test]
    fn parses_chat_and_expands_file_references() {
        let root = unique_temp_dir("mortimmy_tui_parse_chat");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&root.join("README.md"), "hello world\n").unwrap();
        let files = FileIndex::discover(&root).unwrap();

        let parsed = parse("/chat explain @README.md", &files).unwrap();
        match parsed {
            CommandAction::Emit(InputEvent::Command(BrainCommand::Chat(prompt))) => {
                assert!(prompt.contains("explain README.md"));
                assert!(prompt.contains("hello world"));
            }
            other => panic!("unexpected parse result: {other:?}"),
        }

        let _ = std::fs::remove_dir_all(root);
    }
}
