//! The daemon loop: socket, claims, LEDs, input.
//!
//! Threads (std only, no async runtime):
//! - main: owns all state; waits on one channel with a timeout for the next
//!   claim deadline or device retry;
//! - accept: blocks on the listener, forwards new connections;
//! - one reader per connection: forwards lines, EOF, oversize;
//! - input: (re)discovers the evdev source and forwards key events.
//!
//! Only the main thread writes to sockets or the LEDs, so there are no locks.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use macropad_input::{EventSource, InputError, KeyAction, KeyEvent};
use macropad_ipc::{
    ClaimInfo, ClientMsg, ErrorCode, MAX_LINE, PROTOCOL_VERSION, RequestId, ServerMsg, WireColor,
    WireKey, decode_line, encode_line, sniff_req,
};
use macropad_proto::KeyId;

use crate::{ClaimTable, ConnId, Frame, LedSink, UnclaimedPress};

/// Runtime settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Socket path, normally [`socket_path`].
    pub socket: PathBuf,
    /// Claims per connection before `too_many_claims`.
    pub max_claims_per_conn: usize,
    /// Delay between device re-discovery attempts.
    pub reconnect_backoff: Duration,
    /// Reported in `status`.
    pub version: String,
}

impl Config {
    /// Defaults for `socket`.
    #[must_use]
    pub fn new(socket: PathBuf) -> Self {
        Self {
            socket,
            max_claims_per_conn: 32,
            reconnect_backoff: Duration::from_secs(2),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
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
    macropad_ipc::default_socket_path().ok_or(DaemonError::NoRuntimeDir)
}

/// Opens the input device; called again after every disconnect.
pub type InputFactory = Box<dyn FnMut() -> Result<Box<dyn EventSource>, InputError> + Send>;

enum Event {
    Accept(UnixStream),
    Line(ConnId, String),
    TooLong(ConnId),
    Closed(ConnId),
    Key(KeyEvent),
    Shutdown,
}

/// Stops a running daemon.
#[derive(Clone)]
pub struct Handle {
    tx: Sender<Event>,
}

impl Handle {
    /// Ask the daemon to clear the LEDs, remove the socket and return.
    pub fn shutdown(&self) {
        let _ = self.tx.send(Event::Shutdown);
    }
}

struct Conn {
    stream: UnixStream,
    name: Option<String>,
}

/// The daemon, generic over its device seams.
pub struct Daemon<L, U> {
    config: Config,
    claims: ClaimTable,
    leds: L,
    unclaimed: U,
    conns: BTreeMap<ConnId, Conn>,
    next_conn: u64,
    paused: bool,
    device_error: Option<String>,
    last_attempt: Option<Instant>,
}

/// Bind `path`: refuse if a live daemon answers, replace a stale file, make
/// the socket owner-only.
///
/// # Errors
/// [`DaemonError::AlreadyRunning`] or I/O.
pub fn bind(path: &Path) -> Result<UnixListener, DaemonError> {
    if path.exists() {
        if UnixStream::connect(path).is_ok() {
            return Err(DaemonError::AlreadyRunning(path.to_owned()));
        }
        std::fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

impl<L: LedSink + 'static, U: UnclaimedPress + Send + 'static> Daemon<L, U> {
    /// Assemble a daemon. Nothing is opened yet.
    pub fn new(config: Config, leds: L, unclaimed: U) -> Self {
        Self {
            config,
            claims: ClaimTable::default(),
            leds,
            unclaimed,
            conns: BTreeMap::new(),
            next_conn: 0,
            paused: false,
            device_error: None,
            last_attempt: None,
        }
    }

    /// Bind the socket, start the helper threads, and run the main loop on a
    /// new thread. Returns once the socket accepts connections.
    ///
    /// # Errors
    /// [`DaemonError`] when the socket cannot be bound.
    pub fn spawn(self, input: InputFactory) -> Result<(Handle, JoinHandle<()>), DaemonError> {
        let listener = bind(&self.config.socket)?;
        let (tx, rx) = mpsc::channel();
        spawn_acceptor(listener, tx.clone())?;
        spawn_input(input, self.config.reconnect_backoff, tx.clone())?;
        let handle = Handle { tx: tx.clone() };
        let main = thread::Builder::new()
            .name("padd-main".into())
            .spawn(move || self.run(&rx, &tx))?;
        Ok((handle, main))
    }

    fn run(mut self, rx: &Receiver<Event>, tx: &Sender<Event>) {
        self.paint();
        loop {
            let event = match self.wait_for() {
                Some(t) => match rx.recv_timeout(t) {
                    Ok(e) => Some(e),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                },
                None => match rx.recv() {
                    Ok(e) => Some(e),
                    Err(_) => break,
                },
            };
            match event {
                Some(Event::Shutdown) => break,
                Some(e) => self.handle(e, tx),
                None => {}
            }
            self.tick();
        }
        self.leds.shutdown();
        let _ = std::fs::remove_file(&self.config.socket);
        // Clients see EOF at once (their claims are gone with us).
        for c in self.conns.values() {
            let _ = c.stream.shutdown(std::net::Shutdown::Both);
        }
    }

    /// How long the main loop may sleep.
    fn wait_for(&self) -> Option<Duration> {
        let now = Instant::now();
        let deadline = self
            .claims
            .next_deadline()
            .map(|d| d.saturating_duration_since(now));
        let retry = self.device_error.as_ref().map(|_| {
            let since = self.last_attempt.map_or(Duration::MAX, |t| now - t);
            self.config.reconnect_backoff.saturating_sub(since)
        });
        match (deadline, retry) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Expire claims and retry an absent device.
    fn tick(&mut self) {
        let now = Instant::now();
        let expired = self.claims.expire(now);
        for r in &expired {
            self.send(r.owner, &ServerMsg::Expired { claim: r.claim });
        }
        let retry = self.device_error.is_some()
            && self
                .last_attempt
                .is_none_or(|t| now - t >= self.config.reconnect_backoff);
        if !expired.is_empty() || retry {
            self.paint();
        }
    }

    fn paint(&mut self) {
        let frame = if self.paused {
            Frame::default()
        } else {
            self.claims.frame()
        };
        self.last_attempt = Some(Instant::now());
        let result = self.leds.show(&frame);
        let was_present = self.device_error.is_none();
        match result {
            Ok(()) => {
                if self.device_error.take().is_some() {
                    eprintln!("padd: macropad LEDs available");
                }
            }
            Err(e) => {
                if self.device_error.as_deref() != Some(e.as_str()) {
                    eprintln!("padd: macropad LEDs unavailable: {e}");
                }
                self.device_error = Some(e);
            }
        }
        let present = self.device_error.is_none();
        if present != was_present {
            self.broadcast(&ServerMsg::Device { present });
        }
    }

    fn handle(&mut self, event: Event, tx: &Sender<Event>) {
        match event {
            Event::Accept(stream) => self.accept(stream, tx),
            Event::Line(conn, line) => self.line(conn, &line),
            Event::TooLong(conn) => {
                self.send(
                    conn,
                    &ServerMsg::Error {
                        req: None,
                        code: ErrorCode::BadRequest,
                        message: format!("line longer than {MAX_LINE} bytes"),
                    },
                );
                self.close(conn);
            }
            Event::Closed(conn) => self.close(conn),
            Event::Key(key) => self.key(key),
            Event::Shutdown => {}
        }
    }

    fn accept(&mut self, stream: UnixStream, tx: &Sender<Event>) {
        self.next_conn += 1;
        let id = ConnId(self.next_conn);
        let Ok(reader) = stream.try_clone() else {
            return;
        };
        let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
        if spawn_reader(id, reader, tx.clone()).is_ok() {
            self.conns.insert(id, Conn { stream, name: None });
        }
    }

    fn close(&mut self, conn: ConnId) {
        if let Some(c) = self.conns.remove(&conn) {
            let _ = c.stream.shutdown(std::net::Shutdown::Both);
        }
        if !self.claims.drop_conn(conn).is_empty() {
            self.paint();
        }
    }

    fn send(&mut self, conn: ConnId, msg: &ServerMsg) {
        let Some(c) = self.conns.get_mut(&conn) else {
            return;
        };
        let Ok(line) = encode_line(msg) else {
            return;
        };
        if (&c.stream).write_all(line.as_bytes()).is_err() {
            self.close(conn);
        }
    }

    fn broadcast(&mut self, msg: &ServerMsg) {
        let ids: Vec<_> = self
            .conns
            .iter()
            .filter(|(_, c)| c.name.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.send(id, msg);
        }
    }

    fn error(&mut self, conn: ConnId, req: Option<RequestId>, code: ErrorCode, message: String) {
        self.send(conn, &ServerMsg::Error { req, code, message });
    }

    fn line(&mut self, conn: ConnId, line: &str) {
        let msg = match decode_line::<ClientMsg>(line) {
            Ok(msg) => msg,
            Err(e) => {
                return self.error(conn, sniff_req(line), ErrorCode::BadRequest, e.to_string());
            }
        };
        let Some(name) = self.conns.get(&conn).map(|c| c.name.clone()) else {
            return;
        };
        match (name, msg) {
            (None, ClientMsg::Hello { version, client }) => {
                if version != PROTOCOL_VERSION {
                    self.error(
                        conn,
                        None,
                        ErrorCode::Version,
                        format!("padd speaks protocol {PROTOCOL_VERSION}, client {version}"),
                    );
                    return self.close(conn);
                }
                if let Some(c) = self.conns.get_mut(&conn) {
                    c.name = Some(client);
                }
                let msg = ServerMsg::Welcome {
                    version: PROTOCOL_VERSION,
                    device: self.device_error.is_none(),
                    paused: self.paused,
                };
                self.send(conn, &msg);
            }
            (None, _) | (Some(_), ClientMsg::Hello { .. }) => {
                self.error(
                    conn,
                    None,
                    ErrorCode::Version,
                    "send hello exactly once, first".into(),
                );
                self.close(conn);
            }
            (
                Some(name),
                ClientMsg::Claim {
                    req,
                    keys,
                    color,
                    mode: _,
                    timeout_ms,
                },
            ) => {
                self.claim(conn, &name, req, &keys, color, timeout_ms);
            }
            (Some(_), ClientMsg::Release { req, claim }) => {
                if self.claims.release(conn, claim).is_some() {
                    self.send(conn, &ServerMsg::Released { req, claim });
                    self.paint();
                } else {
                    let message = format!("no claim {} on this connection", claim.0);
                    self.error(conn, Some(req), ErrorCode::UnknownClaim, message);
                }
            }
            (Some(_), ClientMsg::Pause { req }) => self.set_paused(conn, req, true),
            (Some(_), ClientMsg::Resume { req }) => self.set_paused(conn, req, false),
            (Some(_), ClientMsg::Status { req }) => {
                let msg = self.status(req);
                self.send(conn, &msg);
            }
        }
    }

    fn claim(
        &mut self,
        conn: ConnId,
        name: &str,
        req: RequestId,
        keys: &[WireKey],
        color: WireColor,
        timeout_ms: Option<u64>,
    ) {
        let keys: Vec<KeyId> = keys.iter().map(|k| k.0).collect();
        if keys.is_empty() {
            return self.error(conn, Some(req), ErrorCode::BadKey, "no keys".into());
        }
        if let Some(dup) = keys
            .iter()
            .enumerate()
            .find(|(i, k)| keys[..*i].contains(k))
        {
            let message = format!("duplicate key {}", dup.1);
            return self.error(conn, Some(req), ErrorCode::BadKey, message);
        }
        if self.claims.count_of(conn) >= self.config.max_claims_per_conn {
            let message = format!(
                "at most {} claims per connection",
                self.config.max_claims_per_conn
            );
            return self.error(conn, Some(req), ErrorCode::TooManyClaims, message);
        }
        let deadline = timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms));
        let outcome = self.claims.claim(conn, &keys, color.0, deadline);
        self.send(
            conn,
            &ServerMsg::Claimed {
                req,
                claim: outcome.id,
            },
        );
        for p in outcome.preempted {
            let wire = |v: Vec<KeyId>| v.into_iter().map(WireKey).collect();
            let msg = ServerMsg::Preempted {
                claim: p.claim,
                lost: wire(p.lost),
                remaining: wire(p.remaining),
                by: name.to_owned(),
            };
            self.send(p.owner, &msg);
        }
        self.paint();
    }

    fn set_paused(&mut self, conn: ConnId, req: RequestId, paused: bool) {
        self.send(conn, &ServerMsg::Ok { req });
        if self.paused != paused {
            self.paused = paused;
            eprintln!("padd: {}", if paused { "paused" } else { "resumed" });
            self.broadcast(&ServerMsg::Pause { paused });
            self.paint();
        }
    }

    fn status(&self, req: RequestId) -> ServerMsg {
        let now = Instant::now();
        let claims = self
            .claims
            .claims()
            .map(|(id, c)| ClaimInfo {
                claim: id,
                client: self
                    .conns
                    .get(&c.owner)
                    .and_then(|c| c.name.clone())
                    .unwrap_or_default(),
                keys: c.keys.iter().copied().map(WireKey).collect(),
                color: WireColor(c.color),
                expires_in_ms: c.deadline.map(|d| {
                    u64::try_from(d.saturating_duration_since(now).as_millis()).unwrap_or(u64::MAX)
                }),
            })
            .collect();
        ServerMsg::Status {
            req,
            daemon: self.config.version.clone(),
            device: self.device_error.is_none(),
            device_error: self.device_error.clone(),
            paused: self.paused,
            clients: self.conns.values().filter(|c| c.name.is_some()).count(),
            claims,
        }
    }

    fn key(&mut self, event: KeyEvent) {
        if event.action != KeyAction::Pressed || self.paused {
            return; // presses only; paused swallows everything
        }
        match self.claims.route(event.key) {
            Some((owner, claim)) => self.send(
                owner,
                &ServerMsg::Press {
                    claim,
                    key: WireKey(event.key),
                },
            ),
            None => self.unclaimed.pressed(event.key),
        }
    }
}

fn spawn_acceptor(listener: UnixListener, tx: Sender<Event>) -> std::io::Result<()> {
    thread::Builder::new()
        .name("padd-accept".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                if tx.send(Event::Accept(stream)).is_err() {
                    return;
                }
            }
        })?;
    Ok(())
}

fn spawn_reader(id: ConnId, stream: UnixStream, tx: Sender<Event>) -> std::io::Result<()> {
    thread::Builder::new()
        .name("padd-conn".into())
        .spawn(move || {
            let mut reader = BufReader::new(stream);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                let limit = u64::try_from(MAX_LINE + 2).unwrap_or(u64::MAX);
                match (&mut reader).take(limit).read_until(b'\n', &mut buf) {
                    Ok(0) => break,
                    Ok(_) if buf.last() != Some(&b'\n') => {
                        if buf.len() > MAX_LINE {
                            let _ = tx.send(Event::TooLong(id));
                        }
                        break;
                    }
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf).into_owned();
                        if tx.send(Event::Line(id, line)).is_err() {
                            return;
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            let _ = tx.send(Event::Closed(id));
        })?;
    Ok(())
}

fn spawn_input(
    mut factory: InputFactory,
    backoff: Duration,
    tx: Sender<Event>,
) -> std::io::Result<()> {
    thread::Builder::new()
        .name("padd-input".into())
        .spawn(move || {
            let mut last_error = String::new();
            loop {
                match factory() {
                    Ok(mut source) => {
                        eprintln!("padd: macropad input grabbed");
                        last_error.clear();
                        loop {
                            match source.next_event(None) {
                                Ok(Some(e)) => {
                                    if tx.send(Event::Key(e)).is_err() {
                                        return;
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    eprintln!("padd: macropad input lost: {e}");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        if msg != last_error {
                            eprintln!("padd: macropad input unavailable: {msg}");
                            last_error = msg;
                        }
                    }
                }
                thread::sleep(backoff);
            }
        })?;
    Ok(())
}
