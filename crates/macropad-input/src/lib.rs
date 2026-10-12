//! Input layer: key presses from the macropad, by physical position.
//!
//! Finds the macropad's keyboard evdev node (`32ac:0013`), grabs it
//! exclusively (`EVIOCGRAB`) so no keycode reaches the compositor, and turns
//! key codes into [`KeyEvent`]s with a [`KeyId`]. Design: ADR 0004.
//!
//! Layer 3 design PR: signatures only, bodies are `todo!()`.

mod evdev_source;
#[cfg(any(test, feature = "fake"))]
pub mod fake;
mod keymap;

use std::time::Duration;

use macropad_proto::KeyId;

pub use evdev_source::{EvdevSource, InputDeviceInfo, find_devices};
pub use keymap::{Keymap, KeymapError};

/// A key changed state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// Which physical key.
    pub key: KeyId,
    /// What happened to it.
    pub action: KeyAction,
}

/// The state change of a [`KeyEvent`]. Autorepeat (`value == 2`) is dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// The key went down.
    Pressed,
    /// The key came up.
    Released,
}

/// Input failures.
#[derive(Debug, thiserror::Error)]
pub enum InputError {
    /// No macropad keyboard interface found.
    #[error("no macropad input device found (is contrib/udev installed?)")]
    NotFound,
    /// Someone else holds the grab (another padd, or a tool like keyd).
    #[error("macropad input device is grabbed by another process")]
    Busy,
    /// The device went away (unplugged, input deck powered off, bootloader).
    #[error("macropad input device disconnected")]
    Disconnected,
    /// OS-level failure.
    #[error("evdev I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

/// A stream of key events. The daemon owns exactly one.
pub trait EventSource {
    /// Wait up to `timeout` (`None`: forever) for the next event.
    /// `Ok(None)` means the timeout passed. Codes the [`Keymap`] does not know
    /// are skipped (and logged), never surfaced.
    ///
    /// # Errors
    /// [`InputError::Disconnected`] when the device is gone; the caller then
    /// re-discovers with backoff.
    fn next_event(&mut self, timeout: Option<Duration>) -> Result<Option<KeyEvent>, InputError>;
}
