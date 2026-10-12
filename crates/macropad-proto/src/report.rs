/// Size in bytes of one Raw HID report, in either direction.
pub const REPORT_LEN: usize = 32;

/// One fixed-size Raw HID report payload.
///
/// This is the unit the transport moves. It excludes the leading HID report id
/// byte that some host APIs require on write; adding that is the transport's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report(pub [u8; REPORT_LEN]);

impl Report {
    /// A report of all zeroes, the padding VIA expects after a command.
    #[must_use]
    pub const fn zeroed() -> Self {
        Self([0; REPORT_LEN])
    }

    /// The raw bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; REPORT_LEN] {
        &self.0
    }
}
