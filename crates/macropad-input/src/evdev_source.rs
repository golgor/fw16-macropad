//! Real [`EventSource`] on `/dev/input/event*` (crate choice: ADR 0004 open
//! question 2; `evdev` 0.13, Apache-2.0 OR MIT, is the candidate).

use std::path::PathBuf;
use std::time::Duration;

use crate::{EventSource, InputError, KeyEvent, Keymap};

/// A candidate input node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDeviceInfo {
    /// `/dev/input/eventN`.
    pub path: PathBuf,
    /// Kernel device name.
    pub name: String,
}

/// Every `32ac:0013` node that reports `EV_KEY` with keyboard keys. The
/// macropad exposes several HID interfaces (keyboard, consumer/system
/// control, Raw HID); all keyboard-like ones are grabbed so media/system
/// codes cannot leak either.
///
/// # Errors
/// [`InputError::Io`] when `/dev/input` cannot be read.
pub fn find_devices() -> Result<Vec<InputDeviceInfo>, InputError> {
    todo!("layer 3: scan /dev/input, match id vendor/product")
}

/// Grabbed macropad input. Dropping it releases the grab.
pub struct EvdevSource {
    keymap: Keymap,
}

impl EvdevSource {
    /// Open and `EVIOCGRAB` every node in `devices`.
    ///
    /// # Errors
    /// [`InputError::Busy`] if any grab fails; nodes already grabbed are
    /// released again, so the call is all-or-nothing.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "layer 3 design stub; the implementation stores it"
    )]
    pub fn open_grabbed(_devices: &[InputDeviceInfo], keymap: Keymap) -> Result<Self, InputError> {
        todo!("layer 3: open + EVIOCGRAB each node, keep {keymap:?}")
    }

    /// The keymap in use.
    #[must_use]
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }
}

impl EventSource for EvdevSource {
    fn next_event(&mut self, _timeout: Option<Duration>) -> Result<Option<KeyEvent>, InputError> {
        todo!("layer 3: poll fds, read EV_KEY, map, drop repeats")
    }
}
