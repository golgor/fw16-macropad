//! Wire types for the `padd` socket protocol. Spec: `docs/protocol.md`.
//!
//! Like `macropad-proto` for HID, this crate owns every byte on the socket and
//! does no I/O. One JSON object per line, UTF-8, `\n` terminated. Both the
//! daemon and `macropad-client` depend on it, so the two cannot drift.
//!
//! Layer 3 design PR: types and signatures are final candidates, bodies are
//! `todo!()`.

use macropad_proto::{KeyId, Rgb};
use serde::{Deserialize, Serialize};

/// Protocol version spoken by this build. Sent in `hello`/`welcome`; a daemon
/// refuses a client whose major version differs.
pub const PROTOCOL_VERSION: u32 = 1;

/// Socket file name inside `$XDG_RUNTIME_DIR`.
pub const SOCKET_NAME: &str = "padd.sock";

/// Longest line either side accepts, in bytes. Longer lines close the
/// connection (protects the daemon from a misbehaving client).
pub const MAX_LINE: usize = 4096;

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

/// How a claim's keys look while held. Only `solid` in v1; `blink` and
/// `pulse` are reserved names (open question in ADR 0004).
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
        /// Free-form name for logs and `preempted.by`, e.g. `"notify-agent"`.
        client: String,
    },
    /// Take keys: light them and receive their presses.
    Claim {
        /// Echoed in `claimed` or `error`.
        req: RequestId,
        /// One or more keys; duplicates are an error.
        keys: Vec<WireKey>,
        /// Colour for all keys of the claim.
        color: WireColor,
        /// Defaults to solid.
        #[serde(default)]
        mode: LightMode,
    },
    /// Give a claim back; its keys go dark (or show the claim underneath, see
    /// ADR 0004 "stacking").
    Release {
        /// Echoed in `released` or `error`.
        req: RequestId,
        /// Claim to drop. Only the owning connection may release it.
        claim: ClaimId,
    },
}

/// Daemon to client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Reply to `hello`.
    Welcome {
        /// Daemon's [`PROTOCOL_VERSION`].
        version: u32,
        /// Whether a macropad is attached right now.
        device: bool,
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
    /// A key of one of this connection's claims went down.
    Press {
        /// Which claim owns the key.
        claim: ClaimId,
        /// Which key.
        key: WireKey,
    },
    /// A newer claim took some of this claim's keys. The claim stays alive
    /// for any keys it still holds; `remaining` empty means it is gone.
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
    /// The macropad went away or came back. Claims survive and are
    /// re-applied on reconnect.
    Device {
        /// Attached now?
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
    /// `hello` missing or version mismatch.
    Version,
    /// Key out of range or duplicated.
    BadKey,
    /// Claim id unknown or owned by another connection.
    UnknownClaim,
    /// Per-connection claim limit hit.
    TooManyClaims,
}

/// Decoding failures, for both directions.
#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    /// Line too long.
    #[error("line exceeds {MAX_LINE} bytes")]
    LineTooLong,
    /// Not valid JSON for the expected type.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Encode one message as a line (with trailing `\n`).
///
/// # Errors
/// Never for these types in practice; kept fallible for serde.
pub fn encode_line<T: Serialize>(_msg: &T) -> Result<String, IpcError> {
    todo!("layer 3: serde_json::to_string + '\\n'")
}

/// Decode one line (with or without trailing `\n`).
///
/// # Errors
/// [`IpcError`] on oversize or malformed input.
pub fn decode_line<'a, T: Deserialize<'a>>(_line: &'a str) -> Result<T, IpcError> {
    todo!("layer 3: length check + serde_json::from_str")
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
    fn serialize<S: serde::Serializer>(&self, _s: S) -> Result<S::Ok, S::Error> {
        todo!("layer 3: \"#rrggbb\"")
    }
}

impl<'de> Deserialize<'de> for WireColor {
    fn deserialize<D: serde::Deserializer<'de>>(_d: D) -> Result<Self, D::Error> {
        todo!("layer 3: parse \"#rrggbb\" (names stay a CLI concern)")
    }
}

#[cfg(test)]
mod tests {
    /// Golden lines from docs/protocol.md; enabled with the implementation.
    #[test]
    #[ignore = "layer 3 design: codec not implemented"]
    fn claim_round_trips_doc_example() {
        let line = r##"{"op":"claim","req":1,"keys":[0,1],"color":"#ff0000"}"##;
        let msg: super::ClientMsg = super::decode_line(line).unwrap();
        assert!(matches!(msg, super::ClientMsg::Claim { .. }));
    }
}
