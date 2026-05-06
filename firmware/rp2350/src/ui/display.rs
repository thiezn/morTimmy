use core::fmt::Write;

use heapless::String;
use mortimmy_core::Mode;

pub const LCD_LINE_WIDTH: usize = 16;

/// Two-line LCD frame rendered for the motion controller status display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayFrame {
    pub line0: String<LCD_LINE_WIDTH>,
    pub line1: String<LCD_LINE_WIDTH>,
}

impl Default for DisplayFrame {
    fn default() -> Self {
        Self {
            line0: String::new(),
            line1: String::new(),
        }
    }
}

/// Render the LCD frame used by the motion controller.
pub fn render_motion_controller_frame(mode: Mode) -> DisplayFrame {
    let mut line0 = String::new();
    let mut line1 = String::new();

    let _ = write!(line0, "Motion ctrl");
    let _ = write!(line1, "Mode {}", mode_label(mode));

    DisplayFrame { line0, line1 }
}

const fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Teleop => "tele",
        Mode::Autonomous => "auto",
        Mode::Fault => "fault",
    }
}

#[cfg(test)]
mod tests {
    use super::render_motion_controller_frame;

    #[test]
    fn motion_controller_frame_is_short_and_stable() {
        let frame = render_motion_controller_frame(mortimmy_core::Mode::Teleop);

        assert_eq!(frame.line0.as_str(), "Motion ctrl");
        assert_eq!(frame.line1.as_str(), "Mode tele");
    }
}
