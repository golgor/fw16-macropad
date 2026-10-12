//! `padctl claim`, `pause`, `resume`, `status`: clients of padd.
//!
//! Exit codes of `padctl claim`, stable for scripts:
//!
//! | code | meaning |
//! |------|---------|
//! | 0 | `--once`: a key was pressed (key number on stdout) |
//! | 1 | other error |
//! | 2 | `--timeout` passed / padd expired the claim |
//! | 3 | preempted: another client took all keys of the claim |
//! | 4 | padd not running, stopped while waiting, or protocol mismatch |

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use clap::Args;
use macropad_client::{ClaimSpec, Client, ClientError, ServerMsg, Wait};
use macropad_proto::{KeyId, Rgb};

/// A failure with a specific process exit code; `main` exits with it.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Exit {
    /// Process exit code.
    pub code: u8,
    /// Printed on stderr.
    pub message: String,
}

impl Exit {
    fn err(code: u8, message: impl Into<String>) -> anyhow::Error {
        Self {
            code,
            message: message.into(),
        }
        .into()
    }
}

/// Exit code 4 for "talking to padd failed"; everything else stays 1.
fn client_err(e: ClientError) -> anyhow::Error {
    match e {
        ClientError::NoDaemon(_) | ClientError::Version(_) | ClientError::Disconnected => {
            Exit::err(4, e.to_string())
        }
        other => other.into(),
    }
}

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
    /// Give up after this long (`30s`, `10m`, `1h`, bare seconds). padd also
    /// enforces it, so the keys go dark even if padctl is killed.
    #[arg(long, value_parser = parse_timeout)]
    pub timeout: Option<Duration>,
    /// Client name shown to preempted clients and in `padctl status`.
    #[arg(long, default_value = "padctl")]
    pub name: String,
}

/// `30s`, `10m`, `2h`, `500ms`, or bare seconds.
pub(crate) fn parse_timeout(s: &str) -> Result<Duration, String> {
    let bad = || format!("{s:?} is not a duration like 30s, 10m, 1h");
    let split = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let n: u64 = num.parse().map_err(|_| bad())?;
    let d = match unit {
        "" | "s" => Duration::from_secs(n),
        "ms" => Duration::from_millis(n),
        "m" => Duration::from_secs(n * 60),
        "h" => Duration::from_secs(n * 3600),
        _ => return Err(bad()),
    };
    if d.is_zero() {
        return Err(bad());
    }
    Ok(d)
}

fn connect(socket: Option<&Path>, name: &str) -> Result<Client> {
    match socket {
        Some(p) => Client::connect(p, name),
        None => Client::connect_default(name),
    }
    .map_err(client_err)
}

/// Run `padctl claim`.
///
/// # Errors
/// [`Exit`] with the codes above.
pub fn run_claim(args: &ClaimArgs, socket: Option<&Path>, out: &mut impl Write) -> Result<()> {
    let mut client = connect(socket, &args.name)?;
    let spec = ClaimSpec::new(args.keys.clone(), args.color).with_timeout(args.timeout);
    let claim = client.claim(&spec).map_err(client_err)?;
    // Client-side deadline a little after padd's, in case padd stalls.
    let deadline = args.timeout.map(|t| t + Duration::from_secs(1));
    if args.once {
        return match client.wait_press(claim, deadline).map_err(client_err)? {
            Wait::Pressed(key) => {
                writeln!(out, "{key}")?;
                Ok(())
            }
            Wait::TimedOut | Wait::Expired => Err(Exit::err(2, "timed out")),
            Wait::Preempted { by } => Err(Exit::err(3, format!("preempted by {by}"))),
        };
    }
    let started = std::time::Instant::now();
    loop {
        let left = deadline.map(|d| d.saturating_sub(started.elapsed()));
        let Some(msg) = client.next_event(left).map_err(client_err)? else {
            return Err(Exit::err(2, "timed out"));
        };
        writeln!(out, "{}", serde_json::to_string(&msg)?)?;
        out.flush()?;
        match msg {
            ServerMsg::Expired { claim: c } if c == claim => return Err(Exit::err(2, "timed out")),
            ServerMsg::Preempted {
                claim: c,
                remaining,
                by,
                ..
            } if c == claim && remaining.is_empty() => {
                return Err(Exit::err(3, format!("preempted by {by}")));
            }
            _ => {}
        }
    }
}

/// Run `padctl pause` / `resume`.
///
/// # Errors
/// Exit 4 when padd is not running.
pub fn run_pause(paused: bool, socket: Option<&Path>) -> Result<()> {
    let mut client = connect(socket, "padctl")?;
    if paused {
        client.pause()
    } else {
        client.resume()
    }
    .map_err(client_err)
}

/// Run `padctl status`.
///
/// # Errors
/// Exit 4 when padd is not running.
pub fn run_status(json: bool, socket: Option<&Path>, out: &mut impl Write) -> Result<()> {
    let mut client = connect(socket, "padctl")?;
    let s = client.status().map_err(client_err)?;
    if json {
        let claims = serde_json::to_value(&s.claims)?;
        let v = serde_json::json!({
            "daemon": s.daemon, "device": s.device, "device_error": s.device_error,
            "paused": s.paused, "clients": s.clients, "claims": claims,
        });
        writeln!(out, "{v}")?;
        return Ok(());
    }
    writeln!(
        out,
        "padd: {} ({})",
        s.daemon,
        if s.paused { "paused" } else { "running" }
    )?;
    match &s.device_error {
        None => writeln!(out, "device: present")?,
        Some(e) => writeln!(out, "device: unavailable ({e})")?,
    }
    writeln!(out, "clients: {}", s.clients)?;
    for c in &s.claims {
        let keys: Vec<_> = c.keys.iter().map(|k| k.0.to_string()).collect();
        let Rgb { r, g, b } = c.color.0;
        write!(
            out,
            "claim {}: {} keys [{}] #{r:02x}{g:02x}{b:02x}",
            c.claim.0,
            c.client,
            keys.join(",")
        )?;
        if let Some(ms) = c.expires_in_ms {
            write!(out, " expires in {}s", ms.div_ceil(1000))?;
        }
        writeln!(out)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_timeouts() {
        assert_eq!(parse_timeout("30s"), Ok(Duration::from_secs(30)));
        assert_eq!(parse_timeout("10m"), Ok(Duration::from_secs(600)));
        assert_eq!(parse_timeout("1h"), Ok(Duration::from_secs(3600)));
        assert_eq!(parse_timeout("250ms"), Ok(Duration::from_millis(250)));
        assert_eq!(parse_timeout("5"), Ok(Duration::from_secs(5)));
        for bad in ["", "m", "0s", "5d", "-1s", "1.5m"] {
            assert!(parse_timeout(bad).is_err(), "{bad}");
        }
    }
}
