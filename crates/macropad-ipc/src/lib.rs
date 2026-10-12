//! Wire types for the `padd` socket protocol. Spec: `docs/protocol.md`.
//!
//! Like `macropad-proto` for HID, this crate owns every byte on the socket and
//! does no I/O. One JSON object per line, UTF-8, `\n` terminated. Both the
//! daemon and `macropad-client` depend on it, so the two cannot drift.

use std::path::PathBuf;

use macropad_proto::{KeyId, Rgb};
use serde::{Deserialize, Serialize};

/// Protocol version spoken by this build. Sent in `hello`/`welcome`; the
/// daemon refuses a client whose version differs.
pub const PROTOCOL_VERSION: u32 = 1;

/// Socket file name inside `$XDG_RUNTIME_DIR`.
pub const SOCKET_NAME: &str = "padd.sock";

/// Longest line either side accepts, in bytes (without the `\n`).
pub const MAX_LINE: usize = 4096;

/// `$XDG_RUNTIME_DIR/padd.sock`, or `None` when the variable is unset.
#[must_use]
pub fn default_socket_path() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|d| !d.is_empty())
        .map(|d| PathBuf::from(d).join(SOCKET_NAME))
}

/// Daemon-assigned claim handle, unique for the daemon's lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClaimId(pub u64);

/// Client-chosen correlation id, echoed in the direct reply to a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(pub u64);

/// A key on the wire: integer `0..24`, validated on deserialize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WireKey(pub KeyId);

/// A colour on the wire: `"#rrggbb"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WireColor(pub Rgb);

/// How a claim's keys look while held. Only `solid` in v1; blink and pulse
/// are planned as host-timed modes (ADR 0004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightMode {
    /// Steady colour.
    #[default]
    Solid,
}

/// Client to daemon, one per line, tagged by `"op"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ClientMsg {
    /// First message on every connection.
    Hello {
        /// [`PROTOCOL_VERSION`] the client speaks.
        version: u32,
        /// Free-form name for logs, `status` and `preempted.by`.
        client: String,
    },
    /// Take keys: light them and receive their presses.
    Claim {
        /// Echoed in `claimed` or `error`.
        req: RequestId,
        /// One or more distinct keys.
        keys: Vec<WireKey>,
        /// Colour for all keys of the claim.
        color: WireColor,
        /// Defaults to solid.
        #[serde(default)]
        mode: LightMode,
        /// padd releases the claim after this long and sends `expired`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    /// Give a claim back; its keys go dark.
    Release {
        /// Echoed in `released` or `error`.
        req: RequestId,
        /// Claim to drop. Only the owning connection may release it.
        claim: ClaimId,
    },
    /// Hide all lights and swallow presses; claims are kept.
    Pause {
        /// Echoed in `ok`.
        req: RequestId,
    },
    /// Undo `pause`.
    Resume {
        /// Echoed in `ok`.
        req: RequestId,
    },
    /// Ask for daemon state.
    Status {
        /// Echoed in `status`.
        req: RequestId,
    },
}

/// One claim as reported by `status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimInfo {
    /// Handle.
    pub claim: ClaimId,
    /// Owner's `hello.client`.
    pub client: String,
    /// Keys held.
    pub keys: Vec<WireKey>,
    /// Colour.
    pub color: WireColor,
    /// Time left before expiry, if the claim has a timeout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in_ms: Option<u64>,
}

/// Daemon to client, tagged by `"event"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Reply to `hello`.
    Welcome {
        /// Daemon's [`PROTOCOL_VERSION`].
        version: u32,
        /// Whether the macropad's LEDs are reachable right now.
        device: bool,
        /// Whether padd is paused.
        paused: bool,
    },
    /// Reply to `claim`.
    Claimed {
        /// From the request.
        req: RequestId,
        /// Handle for `release` and in `press`.
        claim: ClaimId,
    },
    /// Reply to `release`.
    Released {
        /// From the request.
        req: RequestId,
        /// The released claim.
        claim: ClaimId,
    },
    /// Reply to `pause`/`resume`.
    Ok {
        /// From the request.
        req: RequestId,
    },
    /// Reply to `status`.
    Status {
        /// From the request.
        req: RequestId,
        /// Daemon version string.
        daemon: String,
        /// LEDs reachable.
        device: bool,
        /// Why not, if not.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        device_error: Option<String>,
        /// Paused.
        paused: bool,
        /// Connected clients (after `hello`).
        clients: usize,
        /// All claims, oldest first.
        claims: Vec<ClaimInfo>,
    },
    /// A key of one of this connection's claims went down.
    Press {
        /// Which claim owns the key.
        claim: ClaimId,
        /// Which key.
        key: WireKey,
    },
    /// A newer claim took some of this claim's keys. `remaining` empty means
    /// the claim is gone.
    Preempted {
        /// The affected claim.
        claim: ClaimId,
        /// Keys lost.
        lost: Vec<WireKey>,
        /// Keys still held.
        remaining: Vec<WireKey>,
        /// `client` name from the preempting connection's `hello`.
        by: String,
    },
    /// The claim's `timeout_ms` passed; it is gone.
    Expired {
        /// The claim.
        claim: ClaimId,
    },
    /// Pause state changed (sent to every client).
    Pause {
        /// Paused now?
        paused: bool,
    },
    /// The macropad went away or came back (sent to every client). Claims
    /// survive and are repainted on return.
    Device {
        /// Reachable now?
        present: bool,
    },
    /// A request failed.
    Error {
        /// From the request, if the line parsed far enough to have one.
        req: Option<RequestId>,
        /// Machine-readable.
        code: ErrorCode,
        /// Human-readable.
        message: String,
    },
}

/// Stable error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Line was not valid JSON or not a known message.
    BadRequest,
    /// `hello` missing, repeated, or version mismatch.
    Version,
    /// Key list empty or duplicated.
    BadKey,
    /// Claim id unknown or owned by another connection.
    UnknownClaim,
    /// Per-connection claim limit hit.
    TooManyClaims,
}

/// Codec failures, for both directions.
#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    /// Line too long.
    #[error("line exceeds {MAX_LINE} bytes")]
    LineTooLong,
    /// Not valid JSON for the expected type.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Encode one message as a line, with trailing `\n`.
///
/// # Errors
/// [`IpcError::Json`] (not expected for these types).
pub fn encode_line<T: Serialize>(msg: &T) -> Result<String, IpcError> {
    let mut line = serde_json::to_string(msg)?;
    line.push('\n');
    Ok(line)
}

/// Decode one line (trailing `\n`/`\r\n` allowed).
///
/// # Errors
/// [`IpcError`] on oversize or malformed input.
pub fn decode_line<'a, T: Deserialize<'a>>(line: &'a str) -> Result<T, IpcError> {
    let line = line.trim_end_matches(['\n', '\r']);
    if line.len() > MAX_LINE {
        return Err(IpcError::LineTooLong);
    }
    Ok(serde_json::from_str(line)?)
}

/// Best-effort `req` from a line that failed to decode, for error replies.
#[must_use]
pub fn sniff_req(line: &str) -> Option<RequestId> {
    #[derive(Deserialize)]
    struct Req {
        req: RequestId,
    }
    serde_json::from_str::<Req>(line.trim_end())
        .ok()
        .map(|r| r.req)
}

impl Serialize for WireKey {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(self.0.index())
    }
}

impl<'de> Deserialize<'de> for WireKey {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let n = u8::deserialize(d)?;
        KeyId::new(n).map(Self).map_err(serde::de::Error::custom)
    }
}

impl Serialize for WireColor {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let Rgb { r, g, b } = self.0;
        s.serialize_str(&format!("#{r:02x}{g:02x}{b:02x}"))
    }
}

impl<'de> Deserialize<'de> for WireColor {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let bad = || serde::de::Error::custom(format!("{s:?} is not \"#rrggbb\""));
        let hex = s.strip_prefix('#').ok_or_else(bad)?;
        if hex.len() != 6 || !hex.is_ascii() {
            return Err(bad());
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| bad());
        Ok(Self(Rgb {
            r: byte(0)?,
            g: byte(2)?,
            b: byte(4)?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(i: u8) -> WireKey {
        WireKey(KeyId::new(i).unwrap())
    }
    const RED: WireColor = WireColor(Rgb { r: 255, g: 0, b: 0 });

    /// Every example line in docs/protocol.md, decoded and re-encoded
    /// byte for byte.
    #[test]
    fn doc_examples_round_trip() {
        let client = [
            r#"{"op":"hello","version":1,"client":"notify-agent"}"#,
            r##"{"op":"claim","req":1,"keys":[0,1],"color":"#ff0000","mode":"solid"}"##,
            r##"{"op":"claim","req":2,"keys":[23],"color":"#00ff00","mode":"solid","timeout_ms":600000}"##,
            r#"{"op":"release","req":3,"claim":17}"#,
            r#"{"op":"pause","req":4}"#,
            r#"{"op":"resume","req":5}"#,
            r#"{"op":"status","req":6}"#,
        ];
        for line in client {
            let msg: ClientMsg = decode_line(line).unwrap();
            assert_eq!(encode_line(&msg).unwrap(), format!("{line}\n"));
        }
        let server = [
            r#"{"event":"welcome","version":1,"device":true,"paused":false}"#,
            r#"{"event":"claimed","req":1,"claim":17}"#,
            r#"{"event":"released","req":3,"claim":17}"#,
            r#"{"event":"ok","req":4}"#,
            r#"{"event":"press","claim":17,"key":1}"#,
            r#"{"event":"preempted","claim":17,"lost":[1],"remaining":[0],"by":"calendar"}"#,
            r#"{"event":"expired","claim":18}"#,
            r#"{"event":"pause","paused":true}"#,
            r#"{"event":"device","present":false}"#,
            r#"{"event":"error","req":7,"code":"bad_key","message":"duplicate key 3"}"#,
            r##"{"event":"status","req":6,"daemon":"0.1.0","device":true,"paused":false,"clients":1,"claims":[{"claim":17,"client":"notify-agent","keys":[0,1],"color":"#ff0000","expires_in_ms":59000}]}"##,
        ];
        for line in server {
            let msg: ServerMsg = decode_line(line).unwrap();
            assert_eq!(encode_line(&msg).unwrap(), format!("{line}\n"));
        }
    }

    #[test]
    fn claim_defaults_mode_and_timeout() {
        let msg: ClientMsg =
            decode_line(r##"{"op":"claim","req":1,"keys":[0,1],"color":"#FF0000"}"##).unwrap();
        assert_eq!(
            msg,
            ClientMsg::Claim {
                req: RequestId(1),
                keys: vec![k(0), k(1)],
                color: RED,
                mode: LightMode::Solid,
                timeout_ms: None,
            }
        );
    }

    #[test]
    fn rejects_bad_keys_colours_and_oversize() {
        assert!(
            decode_line::<ClientMsg>(r##"{"op":"claim","req":1,"keys":[24],"color":"#ff0000"}"##)
                .is_err()
        );
        assert!(
            decode_line::<ClientMsg>(r#"{"op":"claim","req":1,"keys":[0],"color":"red"}"#).is_err()
        );
        assert!(decode_line::<ClientMsg>(r#"{"op":"nope","req":1}"#).is_err());
        let long = format!(
            r#"{{"op":"hello","version":1,"client":"{}"}}"#,
            "x".repeat(MAX_LINE)
        );
        assert!(matches!(
            decode_line::<ClientMsg>(&long),
            Err(IpcError::LineTooLong)
        ));
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let msg: ClientMsg = decode_line(r#"{"op":"status","req":1,"future":true}"#).unwrap();
        assert_eq!(msg, ClientMsg::Status { req: RequestId(1) });
    }

    #[test]
    fn sniffs_req_from_bad_requests() {
        assert_eq!(sniff_req(r#"{"op":"nope","req":9}"#), Some(RequestId(9)));
        assert_eq!(sniff_req("garbage"), None);
    }
}
