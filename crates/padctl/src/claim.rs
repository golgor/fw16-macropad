//! `padctl claim` (layer 3 design; not wired into [`crate::Cmd`] until the
//! client is implemented, so no command exists that does not work).
//!
//! Exit codes, stable for scripts:
//!
//! | code | meaning |
//! |------|---------|
//! | 0 | `--once`: a key was pressed (key printed on stdout). Streaming: claim ended normally |
//! | 1 | other error |
//! | 2 | `--timeout` passed |
//! | 3 | preempted: another client took all keys of the claim |
//! | 4 | padd not running / protocol version mismatch |

use clap::Args;
use macropad_proto::{KeyId, Rgb};

/// Arguments of `padctl claim`.
#[derive(Debug, Args)]
pub struct ClaimArgs {
    /// Colour, as for `light color`.
    #[arg(long, short, value_parser = crate::parse_rgb)]
    pub color: Rgb,
    /// Keys 0-23.
    #[arg(required = true, value_parser = crate::parse_key)]
    pub keys: Vec<KeyId>,
    /// Exit after the first press, printing the key number.
    #[arg(long)]
    pub once: bool,
    /// Give up after this long (`30s`, `10m`, `1h`).
    #[arg(long, value_parser = parse_timeout)]
    pub timeout: Option<std::time::Duration>,
    /// Client name shown to preempted clients.
    #[arg(long, default_value = "padctl")]
    pub name: String,
}

/// `30s`, `10m`, `2h`, or bare seconds.
fn parse_timeout(_s: &str) -> Result<std::time::Duration, String> {
    todo!("layer 3")
}
