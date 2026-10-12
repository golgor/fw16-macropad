//! Framework factory command `0x0B`, bootloader sub-command only.
//!
//! VIA defines `0x0B` as `id_bootloader_jump`; Framework's firmware takes the id
//! over (`keyboards/framework/factory.c`, `handle_factory_command`) and reads
//! byte 1 as a factory sub-command: `0x01` emulate keypress, `0x04` serial,
//! `0x05` BIOS mode, `0x06` factory mode, `0xFE` bootloader jump.
//!
//! This is the ONLY place in the workspace that writes `0x0B`, and it can only
//! write the bootloader sub-command. It is deliberately not a [`crate::Command`]
//! variant, so nothing that handles generic commands (lighting, per-key, fakes
//! that `parse` requests) can produce or route it. Reference:
//! `FrameworkComputer/qmk_hid` `src/via.rs` `bootloader_jump` (`qmk_hid via
//! --bootloader`), which sends `0x0B` with the rest of the report filled with
//! `0xFE`; byte 1 = `0xFE` is what selects the jump.

use crate::report::Report;

const FACTORY_COMMAND: u8 = 0x0B;
const BOOTLOADER_SUB_COMMAND: u8 = 0xFE;

/// Reboot the macropad into the RP2040 UF2 bootloader (`RPI-RP2` drive).
///
/// The firmware does not reply: the device drops off USB and re-enumerates as
/// the bootloader. Send it fire-and-forget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootloaderJump;

impl BootloaderJump {
    /// `[0x0B, 0xFE, 0xFE, ...]`, byte for byte what `qmk_hid via --bootloader`
    /// writes (it pads with `0xFE`; only bytes 0 and 1 are read).
    #[must_use]
    pub fn encode(self) -> Report {
        let mut report = Report([BOOTLOADER_SUB_COMMAND; crate::REPORT_LEN]);
        report.0[0] = FACTORY_COMMAND;
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_qmk_hid_bootloader_jump() {
        let mut expected = [0xFE; 32];
        expected[0] = 0x0B;
        assert_eq!(BootloaderJump.encode().0, expected);
    }
}
