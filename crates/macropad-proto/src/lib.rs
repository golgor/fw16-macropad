//! Pure protocol layer for the Framework 16 RGB Macropad.
//!
//! This crate owns every byte that crosses the wire and nothing else: no I/O,
//! no allocation-heavy work, no device discovery. Higher layers hand it typed
//! [`Command`]s and get typed [`Response`]s back, so the byte layout is decided
//! in exactly one place.
//!
//! Safety rule: Framework's firmware repurposes VIA command `0x0B`
//! (`id_bootloader_jump`) for factory functions, including a jump to the
//! bootloader. [`Command`] has no variant that encodes to `0x0B`, so it cannot
//! be sent by accident through this crate.

mod command;
mod key;
mod report;

pub use command::{Command, ProtoError, ProtocolVersion, Response};
pub use key::{InvalidKeyId, KEY_COUNT, KeyId, Rgb};
pub use report::{REPORT_LEN, Report};

/// USB vendor id of Framework Computer.
pub const VENDOR_ID: u16 = 0x32AC;
/// USB product id of the Laptop 16 RGB Macropad.
pub const PRODUCT_ID: u16 = 0x0013;
/// HID usage page of the QMK Raw HID interface (the one VIA talks to).
pub const RAW_HID_USAGE_PAGE: u16 = 0xFF60;
/// HID usage of the QMK Raw HID interface.
pub const RAW_HID_USAGE: u16 = 0x61;
