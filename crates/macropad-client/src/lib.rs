//! Blocking client for `padd`. Used by `padctl claim`; other languages speak
//! the JSON-lines protocol directly (`docs/protocol.md`).
//!
//! ```no_run
//! # fn main() -> Result<(), macropad_client::ClientError> {
//! use macropad_client::{Client, ClaimSpec, Wait};
//! use macropad_proto::{KeyId, Rgb};
//! use std::time::Duration;
//!
//! let mut client = Client::connect_default("notify")?;
//! let red = Rgb { r: 255, g: 0, b: 0 };
//! let claim = client.claim(&ClaimSpec::new(vec![KeyId::new(0).unwrap(), KeyId::new(1).unwrap()], red))?;
//! match client.wait_press(claim, Some(Duration::from_secs(600)))? {
//!     Wait::Pressed(key) => println!("pressed {key}"),
//!     Wait::TimedOut | Wait::Preempted { .. } => {}
//! }
//! // Dropping the client closes the socket; padd releases the claim.
//! # Ok(()) }
//! ```
//!
//! Layer 3 design PR: signatures only, bodies are `todo!()`.

use std::path::Path;
use std::time::Duration;

use macropad_ipc::{ClaimId, LightMode, ServerMsg};
use macropad_proto::{KeyId, Rgb};

/// What to claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimSpec {
    /// Keys, non-empty, distinct.
    pub keys: Vec<KeyId>,
    /// Colour.
    pub color: Rgb,
    /// Look.
    pub mode: LightMode,
}

impl ClaimSpec {
    /// Solid colour on `keys`.
    #[must_use]
    pub fn new(keys: Vec<KeyId>, color: Rgb) -> Self {
        Self {
            keys,
            color,
            mode: LightMode::Solid,
        }
    }
}

/// Outcome of [`Client::wait_press`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wait {
    /// A key of the claim was pressed.
    Pressed(KeyId),
    /// Timeout passed first.
    TimedOut,
    /// Another client took keys. `remaining` empty: the claim is gone.
    Preempted {
        /// Keys still held.
        remaining: Vec<KeyId>,
        /// Preempting client's name.
        by: String,
    },
}

/// Client failures. `padctl` maps each to a distinct exit code.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Socket missing or refused: padd not running.
    #[error("padd is not running ({0})")]
    NoDaemon(String),
    /// Version mismatch.
    #[error("padd speaks protocol {daemon}, this client {client}")]
    Version {
        /// Daemon's version.
        daemon: u32,
        /// Ours.
        client: u32,
    },
    /// The daemon rejected a request.
    #[error("padd refused: {0}")]
    Refused(String),
    /// The daemon went away mid-session.
    #[error("connection to padd lost")]
    Disconnected,
    /// Socket I/O.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Malformed line from the daemon.
    #[error(transparent)]
    Ipc(#[from] macropad_ipc::IpcError),
}

/// One connection to padd. Claims live as long as the connection.
pub struct Client {
    _private: (),
}

impl Client {
    /// Connect to `$XDG_RUNTIME_DIR/padd.sock` and say hello as `name`.
    ///
    /// # Errors
    /// [`ClientError::NoDaemon`], [`ClientError::Version`].
    pub fn connect_default(_name: &str) -> Result<Self, ClientError> {
        todo!("layer 3")
    }

    /// Connect to an explicit socket path.
    ///
    /// # Errors
    /// As [`Client::connect_default`].
    pub fn connect(_socket: &Path, _name: &str) -> Result<Self, ClientError> {
        todo!("layer 3")
    }

    /// Claim keys; returns once padd confirmed (keys are lit).
    ///
    /// # Errors
    /// [`ClientError::Refused`] on bad keys or limits.
    pub fn claim(&mut self, _spec: &ClaimSpec) -> Result<ClaimId, ClientError> {
        todo!("layer 3")
    }

    /// Release a claim early.
    ///
    /// # Errors
    /// [`ClientError::Refused`] for unknown claims.
    pub fn release(&mut self, _claim: ClaimId) -> Result<(), ClientError> {
        todo!("layer 3")
    }

    /// Block until a key of `claim` is pressed, the claim is preempted, or
    /// `timeout` passes. Events for other claims are queued for
    /// [`Client::next_event`].
    ///
    /// # Errors
    /// [`ClientError::Disconnected`] if padd goes away.
    pub fn wait_press(
        &mut self,
        _claim: ClaimId,
        _timeout: Option<Duration>,
    ) -> Result<Wait, ClientError> {
        todo!("layer 3")
    }

    /// Next raw event from padd (streaming use, `padctl claim` without
    /// `--once`).
    ///
    /// # Errors
    /// [`ClientError::Disconnected`] if padd goes away.
    pub fn next_event(
        &mut self,
        _timeout: Option<Duration>,
    ) -> Result<Option<ServerMsg>, ClientError> {
        todo!("layer 3")
    }
}
