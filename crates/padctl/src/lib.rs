//! `padctl` command logic, generic over [`Backend`] so it runs against real
//! hardware in `main` and against the fake backend in tests.

use std::io::Write;

use anyhow::{Context, Result, bail};
use clap::CommandFactory;
use clap::{Args, Parser, Subcommand};
use macropad_hid::{Backend, HidError, Macropad, Transport};
use macropad_proto::{Effect, KeyId, Rgb};

/// Control the Framework 16 RGB Macropad.
#[derive(Debug, Parser)]
#[command(version = env!("PADCTL_VERSION"), about)]
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
    /// Global lighting for all keys (stock firmware). Not saved unless `light save`.
    #[command(subcommand)]
    Light(Light),
    /// Per-key colour overrides (needs the padctl firmware). Never saved.
    #[command(subcommand)]
    Key(Key),
    /// Reboot the macropad into its UF2 bootloader (RPI-RP2 drive) for
    /// flashing. Uses Framework's factory command 0x0B, like
    /// `qmk_hid via --bootloader`.
    Bootloader {
        /// Do not ask for confirmation.
        #[arg(long, short)]
        yes: bool,
    },
    /// Print a shell completion script to stdout.
    Completions {
        /// Target shell.
        shell: clap_complete::Shell,
    },
    /// Print the man page (roff) to stdout.
    Man,
}

/// `padctl light ...`
#[derive(Debug, Subcommand)]
pub enum Light {
    /// Print effect, brightness, speed, hue and saturation.
    Show,
    /// Light every key in one colour (solid effect).
    Color {
        /// `rrggbb` hex (optional `#`) or one of: red, green, blue, white, off.
        #[arg(value_parser = parse_rgb)]
        color: Rgb,
    },
    /// Set brightness, 0-255.
    Brightness {
        /// Brightness.
        value: u8,
    },
    /// Select an effect by firmware index (0 off, 1 solid colour).
    Effect {
        /// Effect index.
        index: u8,
    },
    /// Set effect speed, 0-255.
    Speed {
        /// Speed.
        value: u8,
    },
    /// Turn the matrix off.
    Off,
    /// Persist current global lighting to EEPROM.
    Save,
}

/// `padctl key ...`
#[derive(Debug, Subcommand)]
pub enum Key {
    /// Override the colour of one or more keys.
    Set {
        /// Colour, as for `light color`.
        #[arg(long, short, value_parser = parse_rgb)]
        color: Rgb,
        /// Key positions 0-23, row-major from the top left.
        #[arg(required = true, value_parser = parse_key)]
        keys: Vec<KeyId>,
    },
    /// Remove overrides so keys show the global effect again.
    Clear(ClearArgs),
    /// While overrides are active: `exclusive` (default) darkens all other
    /// keys, `overlay` leaves the global effect running underneath.
    Mode {
        /// Mode.
        #[arg(value_enum)]
        mode: KeyMode,
    },
}

/// Per-key display mode.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum KeyMode {
    /// Only overridden keys are lit.
    Exclusive,
    /// Overrides on top of the global effect.
    Overlay,
}

/// Arguments of `padctl key clear`.
#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub struct ClearArgs {
    /// Key positions 0-23.
    #[arg(value_parser = parse_key)]
    keys: Vec<KeyId>,
    /// Clear every key.
    #[arg(long)]
    all: bool,
}

fn parse_key(s: &str) -> Result<KeyId, String> {
    let n: u8 = s
        .parse()
        .map_err(|_| format!("{s:?} is not a key number"))?;
    KeyId::new(n).map_err(|e| e.to_string())
}

fn parse_rgb(s: &str) -> Result<Rgb, String> {
    let named = match s {
        "red" => Some((255, 0, 0)),
        "green" => Some((0, 255, 0)),
        "blue" => Some((0, 0, 255)),
        "white" => Some((255, 255, 255)),
        "off" | "black" => Some((0, 0, 0)),
        _ => None,
    };
    if let Some((r, g, b)) = named {
        return Ok(Rgb { r, g, b });
    }
    let hex = s.strip_prefix('#').unwrap_or(s);
    let bad = || format!("{s:?} is not a colour (rrggbb or a name)");
    if hex.len() != 6 || !hex.is_ascii() {
        return Err(bad());
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| bad());
    Ok(Rgb {
        r: byte(0)?,
        g: byte(2)?,
        b: byte(4)?,
    })
}

/// Run one CLI invocation, writing human output to `out`.
///
/// # Errors
/// Any discovery, transport, or protocol failure, or failure to write `out`.
///
/// `confirm` is asked before anything that disconnects the device; it gets a
/// question and returns whether the user agreed.
pub fn run<B: Backend>(
    cli: &Cli,
    backend: &B,
    out: &mut impl Write,
    confirm: &mut dyn FnMut(&str) -> Result<bool>,
) -> Result<()> {
    match &cli.command {
        Cmd::Bootloader { yes } => {
            let pad = Macropad::new(backend.open_first()?);
            if !yes
                && !confirm(
                    "Reboot the macropad into the bootloader? It stops working as a keyboard until flashed or replugged.",
                )?
            {
                bail!("aborted; pass --yes to skip the question");
            }
            pad.jump_to_bootloader()?;
            writeln!(out, "bootloader: jump sent, look for the RPI-RP2 drive")?;
        }
        Cmd::Completions { shell } => {
            clap_complete::generate(*shell, &mut Cli::command(), "padctl", out);
        }
        Cmd::Man => clap_mangen::Man::new(Cli::command()).render(out)?,
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
            match pad.per_key() {
                Ok(leds) => {
                    let info = leds.info();
                    writeln!(out, "per_key: v{} ({} keys)", info.version, info.key_count)?;
                }
                Err(HidError::NoPerKey(reason)) => {
                    writeln!(out, "per_key: unavailable ({reason})")?;
                }
                Err(e) => return Err(e.into()),
            }
        }
        Cmd::Light(light) => run_light(light, &mut Macropad::new(backend.open_first()?), out)?,
        Cmd::Key(key) => {
            let mut pad = Macropad::new(backend.open_first()?);
            let mut leds = pad.per_key()?;
            match key {
                Key::Set { color, keys } => {
                    let colours: Vec<_> = keys.iter().map(|&k| (k, *color)).collect();
                    if let [(k, c)] = colours[..] {
                        leds.set(k, c)?;
                    } else {
                        leds.set_many(&colours)?;
                    }
                }
                Key::Clear(ClearArgs { all: true, .. }) => leds.clear_all()?,
                Key::Mode { mode } => leds.set_exclusive(matches!(mode, KeyMode::Exclusive))?,
                Key::Clear(ClearArgs { keys, .. }) => {
                    for &k in keys {
                        leds.clear(k)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn run_light<T: Transport>(
    light: &Light,
    pad: &mut Macropad<T>,
    out: &mut impl Write,
) -> Result<()> {
    match *light {
        Light::Show => {
            let l = pad.lighting()?;
            writeln!(out, "effect: {}", l.effect.0)?;
            writeln!(out, "brightness: {}", l.brightness)?;
            writeln!(out, "speed: {}", l.speed)?;
            writeln!(out, "hue: {}", l.hue)?;
            writeln!(out, "sat: {}", l.sat)?;
        }
        Light::Color { color } => pad.set_solid(color)?,
        Light::Brightness { value } => pad.set_brightness(value)?,
        Light::Effect { index } => pad
            .set_effect(Effect(index))
            .with_context(|| format!("effect {index}"))?,
        Light::Speed { value } => pad.set_speed(value)?,
        Light::Off => pad.set_effect(Effect::OFF)?,
        Light::Save => pad.save_lighting()?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_colours() {
        assert_eq!(
            parse_rgb("#ff8000"),
            Ok(Rgb {
                r: 255,
                g: 128,
                b: 0
            })
        );
        assert_eq!(parse_rgb("00FF00"), Ok(Rgb { r: 0, g: 255, b: 0 }));
        assert_eq!(parse_rgb("red"), Ok(Rgb { r: 255, g: 0, b: 0 }));
        assert!(parse_rgb("fff").is_err());
        assert!(parse_rgb("gg0000").is_err());
        assert!(parse_rgb("ééé").is_err());
    }

    #[test]
    fn parses_keys() {
        assert_eq!(parse_key("23").map(KeyId::index), Ok(23));
        assert!(parse_key("24").is_err());
        assert!(parse_key("x").is_err());
    }
}
