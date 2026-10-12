//! Linux key code to [`KeyId`].
//!
//! Framework's stock keymap cannot be used: on the numpad layer three key
//! pairs share a code (`KP_0`, `KP_PLUS`, `KP_ENTER`), and on the Num-Lock-off
//! layer the RGB keys are consumed inside the firmware and never reach evdev.
//! So the mapping targets a firmware layer that sends 24 distinct codes (ADR
//! 0004, open question 1). [`Keymap::padctl_v3`] is that table.

use macropad_proto::{KEY_COUNT, KeyId};

/// A bijection between 24 Linux key codes and the 24 keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    codes: [u16; KEY_COUNT as usize],
}

/// Why a code table is not a valid [`Keymap`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KeymapError {
    /// Two keys share a code, so presses would be ambiguous.
    #[error("key code {0} is used by more than one key")]
    Duplicate(u16),
}

impl Keymap {
    /// Build from codes indexed by [`KeyId::index`].
    ///
    /// # Errors
    /// [`KeymapError::Duplicate`] when codes are not distinct.
    pub fn new(_codes: [u16; KEY_COUNT as usize]) -> Result<Self, KeymapError> {
        todo!("layer 3: check distinct")
    }

    /// The single host layer of the padctl firmware v3 (proposed:
    /// `KC_F13..KC_F24` for keys 0-11, `KC_INTERNATIONAL_1..` / `KC_LANG*`
    /// for 12-23; final list decided with the firmware change).
    #[must_use]
    pub fn padctl_v3() -> Self {
        todo!("layer 3: table matching firmware/keymaps/padctl")
    }

    /// Which key sends `code`, if any.
    #[must_use]
    pub fn key_of(&self, _code: u16) -> Option<KeyId> {
        todo!("layer 3")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "layer 3 design: table not filled in"]
    fn padctl_v3_is_a_bijection() {
        let map = super::Keymap::padctl_v3();
        assert!(
            macropad_proto::KeyId::all()
                .all(|k| map.key_of(map.codes[usize::from(k.index())]) == Some(k))
        );
    }
}
