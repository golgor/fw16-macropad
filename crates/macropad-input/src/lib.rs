//! Input layer: key presses from the macropad, by physical position.
//!
//! Placeholder for layer 4. The plan is to open the macropad's evdev node
//! (matched by `32ac:0013`), grab it so keys never reach the compositor, and
//! translate scancodes to [`KeyId`]. Keys carry no static meaning; whatever is
//! running on the host decides what a press means at that moment.
//!
//! Only the event shape lives here for now, so later layers can be designed
//! against it.

use macropad_proto::KeyId;

/// A key changed state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    /// Which physical key.
    pub key: KeyId,
    /// What happened to it.
    pub action: KeyAction,
}

/// The state change of a [`KeyEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// The key went down.
    Pressed,
    /// The key came up.
    Released,
}
