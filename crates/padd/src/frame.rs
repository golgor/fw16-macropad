//! What the LEDs should show, and the seam that shows it.

use macropad_proto::{KEY_COUNT, KeyId, Rgb};

/// Desired colour per key; `None` is "not claimed". With the firmware's
/// exclusive mode, unclaimed keys are dark while anything is claimed, and an
/// all-`None` frame clears overrides; padd turns the global effect off, so
/// the pad is dark with no claims.
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

/// Paints frames. The daemon calls it after every change and periodically
/// while the device is absent, always with the full frame.
pub trait LedSink: Send {
    /// Make the LEDs match `frame`, (re)connecting if needed.
    ///
    /// # Errors
    /// A description of why the device is unreachable; the daemon reports
    /// it in `status` and retries.
    fn show(&mut self, frame: &Frame) -> Result<(), String>;

    /// Forget what the device is believed to show, so the next
    /// [`show`](Self::show) repaints everything. Called on resume, because
    /// another writer (`padctl --force`, VIA) may have changed the device.
    fn invalidate(&mut self) {}

    /// Called once on shutdown: leave the pad dark and without overrides.
    fn shutdown(&mut self) {
        let _ = self.show(&Frame::default());
    }
}
