//! `padctl` command logic, generic over [`Backend`] so it runs against real
//! hardware in `main` and against the fake backend in tests.

use std::io::Write;

use anyhow::Result;
use clap::{Parser, Subcommand};
use macropad_hid::{Backend, Macropad};

/// Control the Framework 16 RGB Macropad.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    /// What to do.
    #[command(subcommand)]
    pub command: Cmd,
}

/// Subcommands. Only implemented commands exist; new ones arrive with their layer.
#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// List attached macropads (Raw HID interfaces only).
    List,
    /// Show firmware information for the first attached macropad.
    Info,
}

/// Run one CLI invocation, writing human output to `out`.
///
/// # Errors
/// Any discovery, transport, or protocol failure, or failure to write `out`.
pub fn run<B: Backend>(cli: &Cli, backend: &B, out: &mut impl Write) -> Result<()> {
    match cli.command {
        Cmd::List => {
            for device in backend.devices()? {
                writeln!(
                    out,
                    "{}\t{}\t{}",
                    device.path,
                    device.product.as_deref().unwrap_or("-"),
                    device.serial.as_deref().unwrap_or("-"),
                )?;
            }
        }
        Cmd::Info => {
            let mut pad = Macropad::new(backend.open_first()?);
            let version = pad.protocol_version()?;
            writeln!(out, "via_protocol: {}", version.0)?;
        }
    }
    Ok(())
}
