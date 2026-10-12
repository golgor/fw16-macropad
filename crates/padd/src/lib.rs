//! `padd`: the macropad daemon. Design: ADR 0004, protocol: `docs/protocol.md`.
//!
//! One process owns both device sides: the Raw HID transport (LEDs) and the
//! grabbed evdev node (presses). Clients never touch the device; they claim
//! keys over a Unix socket and get presses back.
//!
//! The core is synchronous and generic over three seams so it runs against
//! fakes in tests:
//! - [`LedSink`]: paints a [`Frame`] (real: `PerKeyLeds`).
//! - `macropad_input::EventSource`: key events.
//! - [`UnclaimedPress`]: what happens to a press nobody claimed (layer 3:
//!   nothing; layer 4: default actions).
//!
//! Layer 3 design PR: signatures only, bodies are `todo!()`.

mod claims;
mod daemon;
mod frame;
mod seam;

pub use claims::{ClaimOutcome, ClaimTable, ConnId, Preemption, Released};
pub use daemon::{Config, Daemon, DaemonError, socket_path};
pub use frame::{Frame, LedSink};
pub use seam::{Ignore, UnclaimedPress};
