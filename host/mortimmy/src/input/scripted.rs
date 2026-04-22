use std::collections::VecDeque;
use std::time::Duration;

use anyhow::{Result, anyhow};

use super::source::{CommandInputSource, InputEvent};

/// Deterministic input backend used by unit tests.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Default)]
pub struct ScriptedInput {
    events: VecDeque<InputEvent>,
}

impl ScriptedInput {
    /// Construct scripted input from a fixed event list.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new<T>(events: impl IntoIterator<Item = T>) -> Self
    where
        T: Into<InputEvent>,
    {
        Self {
            events: events.into_iter().map(Into::into).collect(),
        }
    }
}

impl CommandInputSource for ScriptedInput {
    fn next_event(&mut self) -> Result<InputEvent> {
        self.events
            .pop_front()
            .ok_or_else(|| anyhow!("scripted input exhausted"))
    }

    fn poll_event(&mut self, _timeout: Duration) -> Result<Option<InputEvent>> {
        Ok(self.events.pop_front())
    }
}
