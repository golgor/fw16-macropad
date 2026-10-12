//! What the LEDs should show, and the seam that shows it.

use macropad_proto::{KEY_COUNT, KeyId, Rgb};

/// Desired colour per key; `None` is "not claimed". With the firmware's
/// exclusive mode, unclaimed keys are dark while anything is claimed, and an
/// all-`None` frame clears overrides (pad returns to its global effect, which
/// padd sets to off at startup: ADR 0004, open question 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Frame(pub [Option<Rgb>; KEY_COUNT as usize]);

impl Frame {
    /// No key claimed.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }

    /// Colour of one key.
    #[must_use]
    pub fn get(&self, key: KeyId) -> Option<Rgb> {
        self.0[usize::from(key.index())]
    }
}

/// Paints frames. The daemon calls it after every claim change and after the
/// device re-appears, always with the full frame (idempotent, so a reconnect
/// needs no diffing).
pub trait LedSink {
    /// Error type of the device.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Make the LEDs match `frame`. Implementations may diff against the last
    /// frame to save reports.
    ///
    /// # Errors
    /// Device failures; the daemon treats them as "device gone".
    fn show(&mut self, frame: &Frame) -> Result<(), Self::Error>;
}
