//! `padd` binary: wire real devices into [`padd::Daemon`] and run until
//! SIGTERM/SIGINT. Started by `contrib/systemd/padd.service` (user unit).

use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use macropad_hid::HidapiBackend;
use macropad_input::{EvdevSource, EventSource, Keymap};
use padd::{Config, Daemon, HidLeds, Ignore, socket_path};
use signal_hook::consts::{SIGINT, SIGTERM};

/// Framework 16 RGB Macropad daemon: owns the LEDs and key input, hands out
/// keys to clients over a Unix socket.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// Socket path (default: `$XDG_RUNTIME_DIR/padd.sock`).
    #[arg(long)]
    socket: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let socket = match args.socket {
        Some(s) => s,
        None => socket_path()?,
    };
    let leds = HidLeds::new(HidapiBackend::new().context("hidapi")?);
    let keymap = Keymap::padctl_v3();
    let input = Box::new(move || {
        EvdevSource::discover(&keymap).map(|s| Box::new(s) as Box<dyn EventSource>)
    });
    let (handle, main) = Daemon::new(Config::new(socket.clone()), leds, Ignore)
        .spawn(input)
        .with_context(|| format!("starting padd on {}", socket.display()))?;
    eprintln!("padd: listening on {}", socket.display());

    let mut signals = signal_hook::iterator::Signals::new([SIGTERM, SIGINT])?;
    std::thread::spawn(move || {
        if signals.forever().next().is_some() {
            handle.shutdown();
        }
    });
    main.join()
        .map_err(|_| anyhow::anyhow!("padd main thread panicked"))?;
    eprintln!("padd: stopped");
    Ok(())
}
