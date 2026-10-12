//! Scripted [`EventSource`] for tests.

use std::collections::VecDeque;
use std::time::Duration;

use crate::{EventSource, InputError, KeyEvent};

/// Replays a script; `None` entries simulate a timeout, and the end of the
/// script reports [`InputError::Disconnected`].
#[derive(Debug, Default)]
pub struct FakeEvents {
    script: VecDeque<Option<KeyEvent>>,
}

impl FakeEvents {
    /// A source that yields `script` in order.
    #[must_use]
    pub fn new(script: impl IntoIterator<Item = Option<KeyEvent>>) -> Self {
        Self {
            script: script.into_iter().collect(),
        }
    }
}

impl EventSource for FakeEvents {
    fn next_event(&mut self, _timeout: Option<Duration>) -> Result<Option<KeyEvent>, InputError> {
        self.script.pop_front().ok_or(InputError::Disconnected)
    }
}
