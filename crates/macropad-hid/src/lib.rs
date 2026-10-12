//! Transport layer: find the macropad's Raw HID interface and move [`Report`]s.
//!
//! Two seams, both traits so everything above can run against a fake:
//! - [`Backend`] discovers devices and opens them.
//! - [`Transport`] exchanges one request report for one reply report.
//!
//! [`Macropad`] pairs a transport with the protocol crate so callers speak in
//! typed commands, never bytes. Wire types (`hidapi`) stay inside this crate.

#[cfg(any(test, feature = "fake"))]
pub mod fake;
mod hidapi_backend;

use macropad_proto::{Command, ProtoError, ProtocolVersion, Report, Response};

pub use hidapi_backend::HidapiBackend;

/// Opaque handle naming one device interface, as given by [`Backend::devices`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevicePath(pub(crate) String);

impl std::fmt::Display for DevicePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What discovery knows about a macropad without opening it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Pass back to [`Backend::open`].
    pub path: DevicePath,
    /// USB product string, if the device reports one.
    pub product: Option<String>,
    /// USB serial number, if the device reports one.
    pub serial: Option<String>,
}

/// Transport and discovery failures.
#[derive(Debug, thiserror::Error)]
pub enum HidError {
    /// No macropad Raw HID interface was found.
    #[error("no Framework 16 RGB Macropad found (is contrib/udev installed?)")]
    NotFound,
    /// The device did not reply in time.
    #[error("timed out waiting for the macropad to reply")]
    Timeout,
    /// The OS HID layer failed.
    #[error("HID I/O failed: {0}")]
    Io(String),
    /// The reply was not a valid answer.
    #[error(transparent)]
    Protocol(#[from] ProtoError),
}

/// Moves one request report to the device and returns its reply.
///
/// VIA is strictly request/reply, so one blocking call per command is the
/// whole contract. Implementations add or strip HID report ids themselves.
pub trait Transport {
    /// Send `request` and wait for the matching reply.
    ///
    /// # Errors
    /// [`HidError`] on I/O failure or timeout.
    fn exchange(&mut self, request: &Report) -> Result<Report, HidError>;
}

/// Finds macropads and opens them.
pub trait Backend {
    /// The transport [`Backend::open`] hands out.
    type Transport: Transport;

    /// Every macropad Raw HID interface currently attached.
    ///
    /// # Errors
    /// [`HidError::Io`] when enumeration fails.
    fn devices(&self) -> Result<Vec<DeviceInfo>, HidError>;

    /// Open one device for exchanges.
    ///
    /// # Errors
    /// [`HidError`] when the device is gone or not accessible.
    fn open(&self, path: &DevicePath) -> Result<Self::Transport, HidError>;

    /// Open the only, or first, attached macropad.
    ///
    /// # Errors
    /// [`HidError::NotFound`] when none is attached.
    fn open_first(&self) -> Result<Self::Transport, HidError> {
        let first = self
            .devices()?
            .into_iter()
            .next()
            .ok_or(HidError::NotFound)?;
        self.open(&first.path)
    }
}

/// An open macropad, spoken to in typed commands.
pub struct Macropad<T> {
    transport: T,
}

impl<T: Transport> Macropad<T> {
    /// Wrap an open transport.
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    fn request(&mut self, command: Command) -> Result<Response, HidError> {
        let reply = self.transport.exchange(&command.encode())?;
        Ok(command.decode(&reply)?)
    }

    /// The VIA protocol version the firmware speaks.
    ///
    /// # Errors
    /// [`HidError`] on transport or protocol failure.
    pub fn protocol_version(&mut self) -> Result<ProtocolVersion, HidError> {
        match self.request(Command::GetProtocolVersion)? {
            Response::ProtocolVersion(v) => Ok(v),
        }
    }
}
