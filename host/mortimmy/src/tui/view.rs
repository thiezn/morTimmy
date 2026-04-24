use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::config::LogLevel;
use crate::input::ControlState;

use super::{commands, model::{Model, UiLogEntry}};

const COPY_ALL_LABEL: &str = "[Copy All]";
const COPY_LAST_LABEL: &str = "[Copy Last]";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActivityLayout {
    pub panel_area: Rect,
    pub button_row_area: Rect,
    pub copy_all_button_area: Rect,
    pub copy_last_button_area: Rect,
    pub log_area: Rect,
    pub max_scroll_offset: u16,
}

#[derive(Clone, Copy)]
struct Theme {
    border: Style,
    border_emphasis: Style,
    title: Style,
    accent: Style,
    muted: Style,
    info: Style,
    success: Style,
    warning: Style,
    danger: Style,
    selection: Style,
    placeholder: Style,
}

pub fn view(model: &mut Model, frame: &mut Frame) {
    let theme = theme(model.no_color);
    let sections = main_sections(model, frame.area());
    let activity_layout = activity_panel_layout(model, sections[1]);

    render_summary(model, frame, sections[0], theme);
    render_logs(model, frame, activity_layout, theme);
    if sections[2].height > 0 {
        render_completions(model, frame, sections[2], theme);
    }
    render_input(model, frame, sections[3], theme);

    if model.show_help {
        render_help_popup(model, frame, theme);
    }
}

pub fn activity_layout(model: &Model, frame_area: Rect) -> ActivityLayout {
    let sections = main_sections(model, frame_area);
    activity_panel_layout(model, sections[1])
}

fn render_summary(model: &Model, frame: &mut Frame, area: Rect, theme: Theme) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(area);

    let left = vec![
        key_value_line(theme, "Config", &model.summary.config_path, theme.accent),
        key_value_line(
            theme,
            "Transport",
            &format!(
                "{} -> {}",
                model.summary.transport_label, model.summary.serial_target
            ),
            theme.info,
        ),
        key_value_line(
            theme,
            "Connection",
            &model.summary.connection_status,
            connection_style(theme, &model.summary.connection_status),
        ),
        key_value_line(
            theme,
            "Mode",
            &format!("{:?}", model.summary.desired_mode),
            mode_style(theme, model.summary.desired_mode),
        ),
        key_value_line(
            theme,
            "Drive",
            &describe_control_state(model.summary.control_state),
            theme.accent,
        ),
    ];
    let right = vec![
        key_value_line(theme, "NEXO", &model.summary.nexo_gateway, theme.info),
        key_value_line(theme, "Identity", &model.summary.nexo_client, theme.accent),
        key_value_line(
            theme,
            "Controller Lock",
            &model.summary.controller_selection,
            theme.info,
        ),
        key_value_line(
            theme,
            "Active Controllers",
            &model.summary.active_controllers.len().to_string(),
            if model.summary.active_controllers.is_empty() {
                theme.muted
            } else {
                theme.success
            },
        ),
        key_value_line(
            theme,
            "Controllers",
            &if model.summary.active_controllers.is_empty() {
                "none".to_string()
            } else {
                model
                    .summary
                    .active_controllers
                    .values()
                    .map(|controller| controller.display_name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            },
            theme.accent,
        ),
    ];

    frame.render_widget(
        Paragraph::new(Text::from(left))
            .block(panel_block("Summary", theme.border, theme.title))
            .wrap(Wrap { trim: false }),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(Text::from(right))
            .block(panel_block(
                "Status",
                connection_style(theme, &model.summary.connection_status),
                theme.title,
            ))
            .wrap(Wrap { trim: false }),
        columns[1],
    );
}

pub fn activity_plain_text(logs: &std::collections::VecDeque<UiLogEntry>) -> String {
    logs.iter()
        .map(activity_plain_log_entry)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn last_activity_plain_text(logs: &std::collections::VecDeque<UiLogEntry>) -> Option<String> {
    logs.back().map(activity_plain_log_entry)
}

fn render_logs(model: &Model, frame: &mut Frame, layout: ActivityLayout, theme: Theme) {
    let lines = activity_log_lines(&model.logs, theme);
    let scroll_top = layout
        .max_scroll_offset
        .saturating_sub(model.activity_scroll_offset.min(layout.max_scroll_offset));

    frame.render_widget(
        panel_block("Activity", theme.border, theme.title),
        layout.panel_area,
    );

    if layout.button_row_area.height > 0 {
        frame.render_widget(
            Paragraph::new(activity_button_bar(theme))
                .wrap(Wrap { trim: false }),
            layout.button_row_area,
        );
    }

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .scroll((scroll_top, 0))
            .wrap(Wrap { trim: false }),
        layout.log_area,
    );
}

fn render_completions(model: &Model, frame: &mut Frame, area: Rect, theme: Theme) {
    let items = model
        .completions
        .iter()
        .enumerate()
        .map(|(index, suggestion)| {
            let selected = index == model.selected_completion;
            let line = Line::from(vec![
                Span::styled(
                    if selected { "> " } else { "  " },
                    if selected { theme.selection } else { theme.muted },
                ),
                Span::styled(
                    suggestion.label.clone(),
                    if selected {
                        theme.selection.add_modifier(Modifier::BOLD)
                    } else {
                        theme.accent
                    },
                ),
                Span::styled("  ", theme.muted),
                Span::styled(suggestion.detail.clone(), theme.muted),
            ]);
            let item = ListItem::new(line);
            if selected {
                item.style(theme.selection)
            } else {
                item
            }
        })
        .collect::<Vec<_>>();

    frame.render_widget(
        List::new(items).block(panel_block("Autocomplete", theme.border_emphasis, theme.title)),
        area,
    );
}

fn render_input(model: &Model, frame: &mut Frame, area: Rect, theme: Theme) {
    let title = if model.command_input.is_empty() {
        "Command (/help, Tab for autocomplete)"
    } else {
        "Command"
    };
    let content = if model.command_input.is_empty() {
        Line::from(vec![
            Span::styled("> ", theme.accent),
            Span::styled(
                "Type chat text directly, or use /help, /mode teleop, or @file completion",
                theme.placeholder,
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled("> ", theme.accent),
            Span::raw(model.command_input.as_str()),
        ])
    };
    let input = Paragraph::new(content)
        .block(panel_block(title, theme.border_emphasis, theme.title));
    frame.render_widget(input, area);

    let cursor_x = area.x + 3 + model.cursor.min(area.width.saturating_sub(4) as usize) as u16;
    let cursor_y = area.y + 1;
    frame.set_cursor_position((cursor_x, cursor_y));
}

fn render_help_popup(model: &Model, frame: &mut Frame, theme: Theme) {
    let area = centered_rect(80, 70, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(commands::help_text(model.help_topic.as_deref()))
            .block(panel_block("Help", theme.border_emphasis, theme.title))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn activity_log_lines(logs: &std::collections::VecDeque<UiLogEntry>, theme: Theme) -> Vec<Line<'static>> {
    logs.iter()
        .map(|entry| {
            let level_style = log_level_style(theme, entry.level);
            let level_label = format!("[{:<5}] ", log_level_label(entry.level));
            let mut spans = vec![Span::styled(level_label, level_style)];

            spans.push(Span::styled(entry.message.clone(), Style::default()));
            if entry.repeats > 1 {
                spans.push(Span::styled(
                    format!("  x{}", entry.repeats),
                    theme.muted.add_modifier(Modifier::BOLD),
                ));
            }

            Line::from(spans)
        })
        .collect()
}

fn activity_plain_log_entry(entry: &UiLogEntry) -> String {
    if entry.repeats > 1 {
        format!("[{}] {}  x{}", log_level_label(entry.level), entry.message, entry.repeats)
    } else {
        format!("[{}] {}", log_level_label(entry.level), entry.message)
    }
}

fn describe_control_state(control_state: ControlState) -> String {
    match control_state.drive {
        Some(drive) => format!(
            "forward={} turn={} speed={}",
            drive.forward, drive.turn, drive.speed
        ),
        None => "stopped".to_string(),
    }
}

fn theme(no_color: bool) -> Theme {
    if no_color {
        return Theme {
            border: Style::default(),
            border_emphasis: Style::default().add_modifier(Modifier::BOLD),
            title: Style::default().add_modifier(Modifier::BOLD),
            accent: Style::default().add_modifier(Modifier::BOLD),
            muted: Style::default().add_modifier(Modifier::DIM),
            info: Style::default().add_modifier(Modifier::BOLD),
            success: Style::default().add_modifier(Modifier::BOLD),
            warning: Style::default().add_modifier(Modifier::BOLD),
            danger: Style::default().add_modifier(Modifier::BOLD),
            selection: Style::default().add_modifier(Modifier::REVERSED),
            placeholder: Style::default().add_modifier(Modifier::DIM | Modifier::ITALIC),
        };
    }

    Theme {
        border: Style::default().fg(Color::Rgb(102, 120, 138)),
        border_emphasis: Style::default().fg(Color::Rgb(120, 164, 255)),
        title: Style::default()
            .fg(Color::Rgb(232, 237, 243))
            .add_modifier(Modifier::BOLD),
        accent: Style::default().fg(Color::Rgb(126, 214, 223)),
        muted: Style::default().fg(Color::Rgb(142, 154, 175)),
        info: Style::default().fg(Color::Rgb(105, 184, 255)),
        success: Style::default().fg(Color::Rgb(104, 211, 145)),
        warning: Style::default().fg(Color::Rgb(245, 189, 92)),
        danger: Style::default().fg(Color::Rgb(255, 120, 117)),
        selection: Style::default()
            .fg(Color::Rgb(15, 23, 42))
            .bg(Color::Rgb(126, 214, 223)),
        placeholder: Style::default()
            .fg(Color::Rgb(126, 138, 158))
            .add_modifier(Modifier::ITALIC),
    }
}

fn panel_block<'a>(title: &'a str, border_style: Style, title_style: Style) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Line::from(Span::styled(title, title_style)))
}

fn key_value_line(theme: Theme, label: &str, value: &str, value_style: Style) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<18}"), theme.muted),
        Span::styled(value.to_string(), value_style),
    ])
}

fn connection_style(theme: Theme, status: &str) -> Style {
    let lowered = status.to_ascii_lowercase();
    if lowered.contains("disconnected") || lowered.contains("unavailable") || lowered.contains("failed") {
        theme.warning
    } else if lowered.contains("connecting") || lowered.contains("retrying") {
        theme.info
    } else if lowered.starts_with("connected") {
        theme.success
    } else {
        theme.accent
    }
}

fn mode_style(theme: Theme, mode: mortimmy_core::Mode) -> Style {
    match mode {
        mortimmy_core::Mode::Teleop => theme.success,
        mortimmy_core::Mode::Autonomous => theme.info,
        mortimmy_core::Mode::Fault => theme.danger,
    }
}

fn log_level_style(theme: Theme, level: LogLevel) -> Style {
    match level {
        LogLevel::Trace => theme.muted,
        LogLevel::Debug => theme.accent,
        LogLevel::Info => theme.info,
        LogLevel::Warn => theme.warning,
        LogLevel::Error => theme.danger.add_modifier(Modifier::BOLD),
    }
}

fn log_level_label(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Trace => "TRACE",
        LogLevel::Debug => "DEBUG",
        LogLevel::Info => "INFO",
        LogLevel::Warn => "WARN",
        LogLevel::Error => "ERROR",
    }
}

fn main_sections(model: &Model, area: Rect) -> [Rect; 4] {
    let completion_height = if model.completions.is_empty() {
        0
    } else {
        model.completions.len().min(6) as u16 + 2
    };

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(8),
            Constraint::Length(completion_height),
            Constraint::Length(3),
        ])
        .split(area);

    [sections[0], sections[1], sections[2], sections[3]]
}

fn activity_panel_layout(model: &Model, panel_area: Rect) -> ActivityLayout {
    let inner = block_inner(panel_area);
    if inner.width == 0 || inner.height == 0 {
        return ActivityLayout {
            panel_area,
            ..ActivityLayout::default()
        };
    }

    let button_row_area = Rect::new(inner.x, inner.y, inner.width, 1);
    let log_area = Rect::new(
        inner.x,
        inner.y.saturating_add(1),
        inner.width,
        inner.height.saturating_sub(1),
    );
    let copy_all_button_area = Rect::new(
        button_row_area.x,
        button_row_area.y,
        COPY_ALL_LABEL.len() as u16,
        1,
    );
    let copy_last_button_area = Rect::new(
        copy_all_button_area.x.saturating_add(copy_all_button_area.width + 1),
        button_row_area.y,
        COPY_LAST_LABEL.len() as u16,
        1,
    );

    let total_wrapped_lines = if log_area.width == 0 {
        0
    } else {
        activity_log_lines(&model.logs, theme(model.no_color))
            .iter()
            .map(|line| wrapped_line_height(line, log_area.width))
            .sum::<u16>()
    };
    let max_scroll_offset = total_wrapped_lines.saturating_sub(log_area.height);

    ActivityLayout {
        panel_area,
        button_row_area,
        copy_all_button_area,
        copy_last_button_area,
        log_area,
        max_scroll_offset,
    }
}

fn block_inner(area: Rect) -> Rect {
    Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    )
}

fn activity_button_bar(theme: Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(COPY_ALL_LABEL, theme.info.add_modifier(Modifier::BOLD)),
        Span::styled(" ", theme.muted),
        Span::styled(COPY_LAST_LABEL, theme.accent.add_modifier(Modifier::BOLD)),
        Span::styled("  mouse wheel scrolls activity", theme.muted),
    ])
}

fn wrapped_line_height(line: &Line<'_>, width: u16) -> u16 {
    if width == 0 {
        return 0;
    }

    let line_width = line.width() as u16;
    line_width.max(1).div_ceil(width)
}
