//! `padd`: the macropad daemon. Design: ADR 0004, protocol: `docs/protocol.md`.
//!
//! One process owns both device sides: the Raw HID transport (LEDs) and the
//! grabbed evdev nodes (presses). Clients never touch the device; they claim
//! keys over a Unix socket and get presses back.
//!
//! The core is generic over seams so it runs against fakes in tests:
//! - [`LedSink`]: paints a [`Frame`] (real: [`HidLeds`]).
//! - [`InputFactory`]: opens a `macropad_input::EventSource`.
//! - [`UnclaimedPress`]: what happens to a press nobody claimed (layer 3:
//!   nothing; layer 4: default actions).

mod claims;
mod daemon;
mod frame;
mod leds;
mod seam;

pub use claims::{Claim, ClaimOutcome, ClaimTable, ConnId, Preemption, Released};
pub use daemon::{Config, Daemon, DaemonError, Handle, InputFactory, bind, socket_path};
pub use frame::{Frame, LedSink};
pub use leds::HidLeds;
pub use seam::{Ignore, UnclaimedPress};
