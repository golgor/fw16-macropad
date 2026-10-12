//! Linux key code to [`KeyId`].
//!
//! Framework's stock keymap cannot be used: on the numpad layer three key
//! pairs share a code (`KP_0`, `KP_PLUS`, `KP_ENTER`), and on the Num-Lock-off
//! layer the RGB keys are consumed inside the firmware and never reach evdev.
//! The padctl firmware v3 therefore sends 24 distinct codes from one layer;
//! [`Keymap::padctl_v3`] mirrors `firmware/keymaps/padctl/keymap.c`.

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

/// Linux `input-event-codes.h` values of the padctl v3 layer, by key index.
const PADCTL_V3: [u16; KEY_COUNT as usize] = [
    183, 184, 185, 186, // KEY_F13..KEY_F16
    187, 188, 189, 190, // KEY_F17..KEY_F20
    191, 192, 193, 194, // KEY_F21..KEY_F24
    134, 138, 130, 132, // KEY_OPEN KEY_HELP KEY_PROPS KEY_FRONT (Execute Help Menu Select)
    128, 129, 131, 137, // KEY_STOP KEY_AGAIN KEY_UNDO KEY_CUT
    133, 135, 136, 89, // KEY_COPY KEY_PASTE KEY_FIND KEY_RO (International1)
];

impl Keymap {
    /// Build from codes indexed by [`KeyId::index`].
    ///
    /// # Errors
    /// [`KeymapError::Duplicate`] when codes are not distinct.
    pub fn new(codes: [u16; KEY_COUNT as usize]) -> Result<Self, KeymapError> {
        for (i, c) in codes.iter().enumerate() {
            if codes[..i].contains(c) {
                return Err(KeymapError::Duplicate(*c));
            }
        }
        Ok(Self { codes })
    }

    /// The single host layer of the padctl firmware v3.
    #[must_use]
    pub fn padctl_v3() -> Self {
        Self { codes: PADCTL_V3 }
    }

    /// Which key sends `code`, if any.
    #[must_use]
    pub fn key_of(&self, code: u16) -> Option<KeyId> {
        let i = self.codes.iter().position(|&c| c == code)?;
        KeyId::new(u8::try_from(i).ok()?).ok()
    }

    /// The code `key` sends.
    #[must_use]
    pub fn code_of(&self, key: KeyId) -> u16 {
        self.codes[usize::from(key.index())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padctl_v3_is_a_bijection() {
        let map = Keymap::padctl_v3();
        assert_eq!(Keymap::new(PADCTL_V3), Ok(map.clone()));
        assert!(KeyId::all().all(|k| map.key_of(map.code_of(k)) == Some(k)));
        assert_eq!(map.key_of(183).map(KeyId::index), Some(0));
        assert_eq!(map.key_of(89).map(KeyId::index), Some(23));
        assert_eq!(map.key_of(30), None); // KEY_A
    }

    #[test]
    fn rejects_stock_style_duplicates() {
        let mut codes = PADCTL_V3;
        codes[23] = codes[19];
        assert_eq!(Keymap::new(codes), Err(KeymapError::Duplicate(137)));
    }
}
