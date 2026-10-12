//! Real [`EventSource`] on `/dev/input/event*`, via the `evdev` crate.
//!
//! One reader thread per grabbed node feeds a channel; [`EventSource::next_event`]
//! waits on that channel, so timeouts need no `poll`.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use evdev::{Device, EventSummary};
use macropad_proto::{PRODUCT_ID, VENDOR_ID};

use crate::{EventSource, InputError, KeyAction, KeyEvent, Keymap};

/// A candidate input node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDeviceInfo {
    /// `/dev/input/eventN`.
    pub path: PathBuf,
    /// Kernel device name.
    pub name: String,
}

/// Every `32ac:0013` node that reports key events. The macropad exposes
/// several HID interfaces (keyboard, consumer/system control); all are
/// grabbed so no code can leak to the desktop.
///
/// # Errors
/// Never in practice: unreadable nodes are skipped (they belong to other
/// users or are not ours).
pub fn find_devices() -> Result<Vec<InputDeviceInfo>, InputError> {
    let mut found: Vec<_> = evdev::enumerate()
        .filter(|(_, d)| {
            let id = d.input_id();
            id.vendor() == VENDOR_ID && id.product() == PRODUCT_ID && d.supported_keys().is_some()
        })
        .map(|(path, d)| InputDeviceInfo {
            path,
            name: d.name().unwrap_or("?").to_owned(),
        })
        .collect();
    found.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(found)
}

type Item = Result<KeyEvent, InputError>;

/// Grabbed macropad input. Dropping it closes the nodes, which releases the
/// grab; reader threads exit on their next read error.
pub struct EvdevSource {
    rx: Receiver<Item>,
}

impl EvdevSource {
    /// Open and `EVIOCGRAB` every node in `devices`, then start readers.
    ///
    /// # Errors
    /// [`InputError::NotFound`] for an empty list, [`InputError::Busy`] if any
    /// grab fails (nodes already grabbed are closed again, so the call is
    /// all-or-nothing), [`InputError::Io`] if a node cannot be opened.
    pub fn open_grabbed(devices: &[InputDeviceInfo], keymap: &Keymap) -> Result<Self, InputError> {
        if devices.is_empty() {
            return Err(InputError::NotFound);
        }
        let mut grabbed = Vec::with_capacity(devices.len());
        for info in devices {
            let mut device = Device::open(&info.path)?;
            device.grab().map_err(|_| InputError::Busy)?;
            grabbed.push(device);
        }
        let (tx, rx) = mpsc::channel();
        for device in grabbed {
            let tx = tx.clone();
            let keymap = keymap.clone();
            thread::Builder::new()
                .name("padd-evdev".into())
                .spawn(move || read_loop(device, &keymap, &tx))?;
        }
        Ok(Self { rx })
    }

    /// [`find_devices`] then [`EvdevSource::open_grabbed`].
    ///
    /// # Errors
    /// As both.
    pub fn discover(keymap: &Keymap) -> Result<Self, InputError> {
        Self::open_grabbed(&find_devices()?, keymap)
    }
}

fn read_loop(mut device: Device, keymap: &Keymap, tx: &Sender<Item>) {
    loop {
        let Ok(events) = device.fetch_events() else {
            let _ = tx.send(Err(InputError::Disconnected));
            return;
        };
        for event in events {
            let EventSummary::Key(_, code, value) = event.destructure() else {
                continue;
            };
            let action = match value {
                1 => KeyAction::Pressed,
                0 => KeyAction::Released,
                _ => continue, // autorepeat
            };
            let Some(key) = keymap.key_of(code.code()) else {
                eprintln!(
                    "padd: ignoring unmapped key code {} (firmware not v3?)",
                    code.code()
                );
                continue;
            };
            if tx.send(Ok(KeyEvent { key, action })).is_err() {
                return; // source dropped
            }
        }
    }
}

impl EventSource for EvdevSource {
    fn next_event(&mut self, timeout: Option<Duration>) -> Result<Option<KeyEvent>, InputError> {
        let item = match timeout {
            Some(t) => match self.rx.recv_timeout(t) {
                Ok(item) => item,
                Err(RecvTimeoutError::Timeout) => return Ok(None),
                Err(RecvTimeoutError::Disconnected) => return Err(InputError::Disconnected),
            },
            None => self.rx.recv().map_err(|_| InputError::Disconnected)?,
        };
        item.map(Some)
    }
}
