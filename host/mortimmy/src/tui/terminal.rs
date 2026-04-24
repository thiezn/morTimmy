use std::io::{Stdout, stdout};
use std::panic;
use std::sync::Once;

use anyhow::Result;
use crossterm::{
    cursor::{Hide, Show},
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

pub type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub fn init_terminal() -> Result<TuiTerminal> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

pub fn restore_terminal() -> Result<()> {
    let mut stdout = stdout();
    execute!(stdout, Show, DisableMouseCapture, LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

pub fn install_panic_hook() {
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let previous_hook = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            let _ = restore_terminal();
            previous_hook(panic_info);
        }));
    });
}
