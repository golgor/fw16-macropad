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
//! let keys = vec![KeyId::new(0).unwrap(), KeyId::new(1).unwrap()];
//! let claim = client.claim(&ClaimSpec::new(keys, red))?;
//! match client.wait_press(claim, Some(Duration::from_secs(600)))? {
//!     Wait::Pressed(key) => println!("pressed {key}"),
//!     Wait::TimedOut | Wait::Expired | Wait::Preempted { .. } => {}
//! }
//! // Dropping the client closes the socket; padd releases the claim.
//! # Ok(()) }
//! ```

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

pub use macropad_ipc::{ClaimId, ClaimInfo, LightMode, ServerMsg};
use macropad_ipc::{
    ClientMsg, MAX_LINE, PROTOCOL_VERSION, RequestId, WireColor, WireKey, decode_line,
    default_socket_path, encode_line,
};
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
    /// padd drops the claim after this long and sends `expired`.
    pub timeout: Option<Duration>,
}

impl ClaimSpec {
    /// Solid colour on `keys`, no timeout.
    #[must_use]
    pub fn new(keys: Vec<KeyId>, color: Rgb) -> Self {
        Self {
            keys,
            color,
            mode: LightMode::Solid,
            timeout: None,
        }
    }

    /// Set a daemon-enforced timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Outcome of [`Client::wait_press`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wait {
    /// A key of the claim was pressed.
    Pressed(KeyId),
    /// The client-side timeout passed first.
    TimedOut,
    /// padd expired the claim (`timeout_ms`).
    Expired,
    /// Another client took every key of the claim.
    Preempted {
        /// Preempting client's name.
        by: String,
    },
}

/// Daemon state from [`Client::status`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// padd version.
    pub daemon: String,
    /// LEDs reachable.
    pub device: bool,
    /// Why not.
    pub device_error: Option<String>,
    /// Paused.
    pub paused: bool,
    /// Connected clients.
    pub clients: usize,
    /// Claims.
    pub claims: Vec<ClaimInfo>,
}

/// Client failures. `padctl` maps each to an exit code.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Socket missing or refused: padd not running.
    #[error("padd is not running ({0})")]
    NoDaemon(String),
    /// Version mismatch.
    #[error("padd refused the connection: {0}")]
    Version(String),
    /// The daemon rejected a request.
    #[error("padd refused: {0}")]
    Refused(String),
    /// The daemon went away mid-session (stopped or restarted). Claims are
    /// gone; reconnect and claim again.
    #[error("connection to padd lost")]
    Disconnected,
    /// The daemon sent something this client does not understand.
    #[error("unexpected reply from padd: {0}")]
    Protocol(String),
    /// Socket I/O.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// One connection to padd. Claims live as long as the connection.
pub struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    partial: Vec<u8>,
    queued: VecDeque<ServerMsg>,
    next_req: u64,
    welcome: (bool, bool),
}

/// True when a daemon answers on the default socket.
#[must_use]
pub fn daemon_running() -> bool {
    default_socket_path().is_some_and(|p| UnixStream::connect(p).is_ok())
}

impl Client {
    /// Connect to `$XDG_RUNTIME_DIR/padd.sock` and say hello as `name`.
    ///
    /// # Errors
    /// [`ClientError::NoDaemon`], [`ClientError::Version`].
    pub fn connect_default(name: &str) -> Result<Self, ClientError> {
        let path = default_socket_path()
            .ok_or_else(|| ClientError::NoDaemon("XDG_RUNTIME_DIR is not set".into()))?;
        Self::connect(&path, name)
    }

    /// Connect to an explicit socket path.
    ///
    /// # Errors
    /// As [`Client::connect_default`].
    pub fn connect(socket: &Path, name: &str) -> Result<Self, ClientError> {
        let stream = UnixStream::connect(socket)
            .map_err(|e| ClientError::NoDaemon(format!("{}: {e}", socket.display())))?;
        let writer = stream.try_clone()?;
        let mut client = Self {
            reader: BufReader::new(stream),
            writer,
            partial: Vec::new(),
            queued: VecDeque::new(),
            next_req: 0,
            welcome: (false, false),
        };
        client.send(&ClientMsg::Hello {
            version: PROTOCOL_VERSION,
            client: name.to_owned(),
        })?;
        match client.read(None)? {
            Some(ServerMsg::Welcome { device, paused, .. }) => client.welcome = (device, paused),
            Some(ServerMsg::Error { message, .. }) => return Err(ClientError::Version(message)),
            other => return Err(ClientError::Protocol(format!("{other:?}"))),
        }
        Ok(client)
    }

    /// `(device present, paused)` as of the handshake.
    #[must_use]
    pub fn welcome(&self) -> (bool, bool) {
        self.welcome
    }

    fn send(&mut self, msg: &ClientMsg) -> Result<(), ClientError> {
        let line = encode_line(msg).map_err(|e| ClientError::Protocol(e.to_string()))?;
        self.writer
            .write_all(line.as_bytes())
            .map_err(|e| match e.kind() {
                ErrorKind::BrokenPipe | ErrorKind::ConnectionReset => ClientError::Disconnected,
                _ => ClientError::Io(e),
            })
    }

    /// Read one message from the socket, waiting until `deadline`.
    fn read(&mut self, deadline: Option<Instant>) -> Result<Option<ServerMsg>, ClientError> {
        loop {
            let timeout = match deadline {
                Some(d) => match d.checked_duration_since(Instant::now()) {
                    Some(t) if !t.is_zero() => Some(t),
                    _ => return Ok(None),
                },
                None => None,
            };
            self.reader.get_ref().set_read_timeout(timeout)?;
            match self.reader.read_until(b'\n', &mut self.partial) {
                Ok(0) => return Err(ClientError::Disconnected),
                Ok(_) if self.partial.last() == Some(&b'\n') => {
                    let line = String::from_utf8_lossy(&self.partial).into_owned();
                    self.partial.clear();
                    return decode_line(&line)
                        .map(Some)
                        .map_err(|e| ClientError::Protocol(e.to_string()));
                }
                Ok(_) => {
                    if self.partial.len() > MAX_LINE {
                        return Err(ClientError::Protocol("line too long".into()));
                    }
                }
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    return Ok(None);
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) if e.kind() == ErrorKind::ConnectionReset => {
                    return Err(ClientError::Disconnected);
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Send a request and wait for its reply; other messages are queued.
    fn request(
        &mut self,
        make: impl FnOnce(RequestId) -> ClientMsg,
    ) -> Result<ServerMsg, ClientError> {
        self.next_req += 1;
        let req = RequestId(self.next_req);
        self.send(&make(req))?;
        loop {
            let Some(msg) = self.read(None)? else {
                continue;
            };
            let reply_req = match &msg {
                ServerMsg::Claimed { req, .. }
                | ServerMsg::Released { req, .. }
                | ServerMsg::Ok { req }
                | ServerMsg::Status { req, .. } => Some(*req),
                ServerMsg::Error { req, .. } => *req,
                _ => None,
            };
            if reply_req == Some(req) {
                if let ServerMsg::Error { message, .. } = msg {
                    return Err(ClientError::Refused(message));
                }
                return Ok(msg);
            }
            self.queued.push_back(msg);
        }
    }

    /// Claim keys; returns once padd confirmed (keys are lit).
    ///
    /// # Errors
    /// [`ClientError::Refused`] on bad keys or limits.
    pub fn claim(&mut self, spec: &ClaimSpec) -> Result<ClaimId, ClientError> {
        let timeout_ms = spec
            .timeout
            .map(|t| u64::try_from(t.as_millis()).unwrap_or(u64::MAX));
        match self.request(|req| ClientMsg::Claim {
            req,
            keys: spec.keys.iter().copied().map(WireKey).collect(),
            color: WireColor(spec.color),
            mode: spec.mode,
            timeout_ms,
        })? {
            ServerMsg::Claimed { claim, .. } => Ok(claim),
            other => Err(ClientError::Protocol(format!("{other:?}"))),
        }
    }

    /// Release a claim early.
    ///
    /// # Errors
    /// [`ClientError::Refused`] for unknown claims.
    pub fn release(&mut self, claim: ClaimId) -> Result<(), ClientError> {
        self.request(|req| ClientMsg::Release { req, claim })
            .map(|_| ())
    }

    /// Pause padd (lights hidden, presses swallowed, claims kept).
    ///
    /// # Errors
    /// Connection failures.
    pub fn pause(&mut self) -> Result<(), ClientError> {
        self.request(|req| ClientMsg::Pause { req }).map(|_| ())
    }

    /// Resume padd.
    ///
    /// # Errors
    /// Connection failures.
    pub fn resume(&mut self) -> Result<(), ClientError> {
        self.request(|req| ClientMsg::Resume { req }).map(|_| ())
    }

    /// Daemon state.
    ///
    /// # Errors
    /// Connection failures.
    pub fn status(&mut self) -> Result<Status, ClientError> {
        match self.request(|req| ClientMsg::Status { req })? {
            ServerMsg::Status {
                daemon,
                device,
                device_error,
                paused,
                clients,
                claims,
                ..
            } => Ok(Status {
                daemon,
                device,
                device_error,
                paused,
                clients,
                claims,
            }),
            other => Err(ClientError::Protocol(format!("{other:?}"))),
        }
    }

    /// Block until a key of `claim` is pressed, the claim ends, or `timeout`
    /// passes. Events for other claims stay queued for
    /// [`Client::next_event`].
    ///
    /// # Errors
    /// [`ClientError::Disconnected`] if padd goes away.
    pub fn wait_press(
        &mut self,
        claim: ClaimId,
        timeout: Option<Duration>,
    ) -> Result<Wait, ClientError> {
        let deadline = timeout.map(|t| Instant::now() + t);
        let mut skipped = VecDeque::new();
        let result = loop {
            let msg = match self.queued.pop_front() {
                Some(m) => m,
                None => match self.read(deadline)? {
                    Some(m) => m,
                    None => break Wait::TimedOut,
                },
            };
            match msg {
                ServerMsg::Press { claim: c, key } if c == claim => break Wait::Pressed(key.0),
                ServerMsg::Expired { claim: c } if c == claim => break Wait::Expired,
                ServerMsg::Preempted {
                    claim: c,
                    remaining,
                    by,
                    ..
                } if c == claim && remaining.is_empty() => {
                    break Wait::Preempted { by };
                }
                other => skipped.push_back(other),
            }
        };
        skipped.append(&mut self.queued);
        self.queued = skipped;
        Ok(result)
    }

    /// Next event from padd (streaming use). `Ok(None)` on timeout.
    ///
    /// # Errors
    /// [`ClientError::Disconnected`] if padd goes away.
    pub fn next_event(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<Option<ServerMsg>, ClientError> {
        if let Some(m) = self.queued.pop_front() {
            return Ok(Some(m));
        }
        self.read(timeout.map(|t| Instant::now() + t))
    }
}
