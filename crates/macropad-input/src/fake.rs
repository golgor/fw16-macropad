//! Channel-driven [`EventSource`] for tests: the test holds the sender.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use crate::{EventSource, InputError, KeyEvent};

/// Events pushed by a test. Dropping the sender reads as
/// [`InputError::Disconnected`].
#[derive(Debug)]
pub struct FakeEvents {
    rx: Receiver<KeyEvent>,
}

impl FakeEvents {
    /// A source and the sender that feeds it.
    #[must_use]
    pub fn channel() -> (Sender<KeyEvent>, Self) {
        let (tx, rx) = mpsc::channel();
        (tx, Self { rx })
    }
}

impl EventSource for FakeEvents {
    fn next_event(&mut self, timeout: Option<Duration>) -> Result<Option<KeyEvent>, InputError> {
        match timeout {
            Some(t) => match self.rx.recv_timeout(t) {
                Ok(e) => Ok(Some(e)),
                Err(RecvTimeoutError::Timeout) => Ok(None),
                Err(RecvTimeoutError::Disconnected) => Err(InputError::Disconnected),
            },
            None => self
                .rx
                .recv()
                .map(Some)
                .map_err(|_| InputError::Disconnected),
        }
    }
}
