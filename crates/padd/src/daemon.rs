//! The daemon loop: socket, claims, LEDs, input.
//!
//! Threading (ADR 0004, open question 3): proposed std-only, one input
//! thread (blocking evdev read) and one main thread multiplexing the listener
//! and client sockets with `poll`, joined by a channel. No async runtime.

use std::path::PathBuf;
use std::time::Duration;

use macropad_input::EventSource;

use crate::{ClaimTable, LedSink, UnclaimedPress};

/// Runtime settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Socket path, normally [`socket_path`].
    pub socket: PathBuf,
    /// Claims per connection before `too_many_claims`.
    pub max_claims_per_conn: usize,
    /// Backoff between device re-discovery attempts.
    pub reconnect_backoff: Duration,
}

/// Fatal daemon errors (per-client problems are protocol errors instead).
#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    /// `$XDG_RUNTIME_DIR` unset.
    #[error("XDG_RUNTIME_DIR is not set; padd runs as a user service")]
    NoRuntimeDir,
    /// Another padd is listening on the socket.
    #[error("another padd is already running on {0}")]
    AlreadyRunning(PathBuf),
    /// Socket or OS failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// `$XDG_RUNTIME_DIR/padd.sock`.
///
/// # Errors
/// [`DaemonError::NoRuntimeDir`].
pub fn socket_path() -> Result<PathBuf, DaemonError> {
    todo!("layer 3")
}

/// The daemon, generic over its device seams.
#[allow(dead_code, reason = "layer 3 design: fields used by the implementation")]
pub struct Daemon<L, E, U> {
    config: Config,
    claims: ClaimTable,
    leds: L,
    input: E,
    unclaimed: U,
}

impl<L: LedSink, E: EventSource, U: UnclaimedPress> Daemon<L, E, U> {
    /// Assemble a daemon. Nothing is opened yet.
    pub fn new(config: Config, leds: L, input: E, unclaimed: U) -> Self {
        Self {
            config,
            claims: ClaimTable::default(),
            leds,
            input,
            unclaimed,
        }
    }

    /// Bind the socket (refusing if a live daemon answers on it, replacing a
    /// stale file otherwise), paint a dark frame, and serve until SIGTERM.
    /// On exit: clear overrides, release the grab, remove the socket.
    ///
    /// # Errors
    /// [`DaemonError`] on startup failure.
    pub fn run(self) -> Result<(), DaemonError> {
        drop(self);
        todo!("layer 3")
    }
}
