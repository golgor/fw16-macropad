use std::fmt;

/// Number of physical keys (and RGB LEDs) on the macropad.
pub const KEY_COUNT: u8 = 24;

/// A physical key on the macropad, `0..24`, numbered row-major from the top left
/// (4 columns by 6 rows).
///
/// Keys are identified by position, never by keycode: the host decides at
/// runtime what a key means, so the keymap is not the source of identity.
/// Only [`KeyId::new`] constructs one, so every `KeyId` in the program is valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyId(u8);

/// Returned when a number does not name a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("key id {0} is out of range, expected 0..{KEY_COUNT}")]
pub struct InvalidKeyId(pub u8);

impl KeyId {
    /// Parse a key index. This is the only way to make a `KeyId`.
    ///
    /// # Errors
    /// [`InvalidKeyId`] when `index >= KEY_COUNT`.
    pub const fn new(index: u8) -> Result<Self, InvalidKeyId> {
        if index < KEY_COUNT {
            Ok(Self(index))
        } else {
            Err(InvalidKeyId(index))
        }
    }

    /// The zero-based index.
    #[must_use]
    pub const fn index(self) -> u8 {
        self.0
    }

    /// Every key, in index order.
    pub fn all() -> impl Iterator<Item = Self> {
        (0..KEY_COUNT).map(Self)
    }
}

impl fmt::Display for KeyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A 24-bit colour. Layer 2 decides how this maps to the firmware's HSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_exactly_the_physical_keys() {
        assert_eq!(KeyId::all().count(), usize::from(KEY_COUNT));
        assert!(KeyId::new(23).is_ok());
        assert_eq!(KeyId::new(24), Err(InvalidKeyId(24)));
    }
}
