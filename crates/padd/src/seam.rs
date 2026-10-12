//! Layer 4 seam: presses on unclaimed keys.

use macropad_proto::KeyId;

/// Called for a press nobody claimed. Layer 3 ships [`Ignore`]; layer 4 may
/// add configured default actions here without touching claim logic. The
/// daemon itself never stores client actions.
pub trait UnclaimedPress {
    /// Handle one unclaimed press.
    fn pressed(&mut self, key: KeyId);
}

/// Layer 3 behaviour: unclaimed keys do nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct Ignore;

impl UnclaimedPress for Ignore {
    fn pressed(&mut self, _key: KeyId) {}
}
