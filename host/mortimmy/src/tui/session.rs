use anyhow::Result;
use mortimmy_core::Mode;

use crate::{config::LogLevel, input::ControlState};

pub trait SessionOutput {
    fn log(&mut self, level: LogLevel, message: String) -> Result<()>;
    fn set_connection_status(&mut self, status: String) -> Result<()>;
    fn set_control_state(&mut self, control_state: ControlState) -> Result<()>;
    fn set_desired_mode(&mut self, mode: Mode) -> Result<()>;
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

    fn set_desired_mode(&mut self, _mode: Mode) -> Result<()> {
        Ok(())
    }
}
