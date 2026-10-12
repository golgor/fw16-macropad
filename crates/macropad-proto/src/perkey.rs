//! Our channel-0 sub-protocol, served by `firmware/keymaps/padctl`.
//!
//! Byte layout of every request (32 bytes, zero padded):
//!
//! | byte | meaning                                                   |
//! |------|-----------------------------------------------------------|
//! | 0    | VIA command: `0x07` custom set value, `0x08` custom get   |
//! | 1    | channel `0x00` (VIA `id_custom_channel`)                  |
//! | 2    | value id, one of [`value`]                                |
//! | 3..  | arguments, see [`crate::Command`]                         |
//!
//! The firmware echoes the request. It answers `0xFF` in byte 0 when it does
//! not know the value id or the arguments are invalid, and stock firmware
//! answers `0xFF` to all of channel 0, which is how the probe tells them apart.

use crate::key::{KeyId, Rgb};
use crate::report::REPORT_LEN;

/// VIA channel id for keyboard-specific values.
pub(crate) const CHANNEL: u8 = 0x00;

/// Value ids on channel 0. Mirrors `enum padctl_value_id` in `keymap.c`.
pub(crate) mod value {
    /// Get: magic and version.
    pub const PROBE: u8 = 0x01;
    /// Set: one key `[key, r, g, b]`.
    pub const SET_KEY: u8 = 0x02;
    /// Set: `[count, (key, r, g, b) * count]`.
    pub const SET_MANY: u8 = 0x03;
    /// Set: drop every override.
    pub const CLEAR_ALL: u8 = 0x04;
    /// Set: drop one override `[key]`.
    pub const CLEAR_KEY: u8 = 0x05;
    /// Set: `[exclusive]`, 1 (default) blanks non-overridden keys while any
    /// override is active, 0 overlays overrides on the running effect.
    pub const SET_EXCLUSIVE: u8 = 0x06;
}

/// Magic in probe replies, ASCII `pd`, so an unrelated firmware that happens to
/// answer channel 0 is not mistaken for ours.
pub(crate) const MAGIC: [u8; 2] = *b"pd";

/// Sub-protocol version this crate speaks.
pub const PER_KEY_PROTOCOL_VERSION: u8 = 3;

/// Bytes before the first `(key, r, g, b)` entry of a `SetMany` request.
const SET_MANY_HEADER: usize = 4;

/// Most keys one `SetMany` report carries: `(32 - 4) / 4 = 7`.
pub const MAX_BATCH: usize = (REPORT_LEN - SET_MANY_HEADER) / 4;

/// What the firmware reported on a successful probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerKeyInfo {
    /// Sub-protocol version the firmware speaks.
    pub version: u8,
    /// LEDs the firmware addresses (24 on the macropad).
    pub key_count: u8,
}

/// Up to [`MAX_BATCH`] key colours that fit in one report.
///
/// Built only through [`KeyBatch::chunks`], so a batch is never empty and never
/// overflows the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBatch {
    len: u8,
    entries: [(KeyId, Rgb); MAX_BATCH],
}

impl KeyBatch {
    /// Split any number of key colours into report-sized batches, in order.
    pub fn chunks(colours: &[(KeyId, Rgb)]) -> impl Iterator<Item = Self> + '_ {
        colours.chunks(MAX_BATCH).map(|chunk| {
            let filler = chunk[0];
            let mut entries = [filler; MAX_BATCH];
            entries[..chunk.len()].copy_from_slice(chunk);
            Self {
                // MAX_BATCH is 7, so this always fits.
                len: u8::try_from(chunk.len()).unwrap_or(u8::MAX),
                entries,
            }
        })
    }

    /// The entries in this batch.
    #[must_use]
    pub fn entries(&self) -> &[(KeyId, Rgb)] {
        &self.entries[..usize::from(self.len)]
    }

    /// Inverse of [`KeyBatch::write`]; `None` for an empty, oversized or
    /// out-of-range batch.
    pub(crate) fn parse(args: &[u8]) -> Option<Self> {
        let len = usize::from(args[0]);
        if len == 0 || len > MAX_BATCH {
            return None;
        }
        let mut colours = [(KeyId::new(0).ok()?, Rgb { r: 0, g: 0, b: 0 }); MAX_BATCH];
        for (slot, e) in colours.iter_mut().zip(args[1..].chunks_exact(4)).take(len) {
            *slot = (
                KeyId::new(e[0]).ok()?,
                Rgb {
                    r: e[1],
                    g: e[2],
                    b: e[3],
                },
            );
        }
        Self::chunks(&colours[..len]).next()
    }

    pub(crate) fn write(&self, args: &mut [u8]) {
        args[0] = self.len;
        for (slot, (key, c)) in args[1..].chunks_exact_mut(4).zip(self.entries()) {
            slot.copy_from_slice(&[key.index(), c.r, c.g, c.b]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(i: u8) -> KeyId {
        KeyId::new(i).unwrap()
    }

    #[test]
    fn seven_keys_fit_in_one_report() {
        assert_eq!(MAX_BATCH, 7);
    }

    #[test]
    fn chunks_split_at_seven() {
        let red = Rgb { r: 255, g: 0, b: 0 };
        let all: Vec<_> = KeyId::all().map(|key| (key, red)).collect();
        let lens: Vec<_> = KeyBatch::chunks(&all).map(|b| b.entries().len()).collect();
        assert_eq!(lens, [7, 7, 7, 3]);
        assert_eq!(KeyBatch::chunks(&[]).count(), 0);
        assert_eq!(
            KeyBatch::chunks(&[(k(5), red)]).next().unwrap().entries(),
            &[(k(5), red)]
        );
    }
}
