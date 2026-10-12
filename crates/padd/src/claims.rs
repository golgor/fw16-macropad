//! Who owns which key. Pure state machine: no I/O, no clock, fully testable.
//!
//! Rules (ADR 0004):
//! - A claim is a set of keys plus a colour, owned by one connection.
//! - Newest claim wins: claiming a held key moves it to the new claim and the
//!   old owner gets a [`Preemption`]. Lost keys do not come back when the
//!   newer claim ends (no stacking in v1).
//! - A claim with no keys left is gone.
//! - Disconnect releases every claim of that connection.

use std::collections::BTreeMap;

use macropad_ipc::ClaimId;
use macropad_proto::{KEY_COUNT, KeyId, Rgb};

/// Daemon-local connection handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnId(pub u64);

/// One live claim.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Claim {
    owner: ConnId,
    color: Rgb,
}

/// Every claim, plus the per-key owner index.
#[allow(
    dead_code,
    reason = "layer 3 design: fields used by the implementation"
)]
#[derive(Debug, Default)]
pub struct ClaimTable {
    next_id: u64,
    claims: BTreeMap<ClaimId, Claim>,
    owner_of_key: [Option<ClaimId>; KEY_COUNT as usize],
}

/// Keys taken from an existing claim by a newer one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preemption {
    /// Who to notify.
    pub owner: ConnId,
    /// The claim that lost keys.
    pub claim: ClaimId,
    /// Keys it lost.
    pub lost: Vec<KeyId>,
    /// Keys it still holds (empty: claim is gone).
    pub remaining: Vec<KeyId>,
}

/// Result of [`ClaimTable::claim`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimOutcome {
    /// The new claim.
    pub id: ClaimId,
    /// Older claims that lost keys, one entry per affected claim.
    pub preempted: Vec<Preemption>,
}

/// A claim removed by [`ClaimTable::release`] or [`ClaimTable::drop_conn`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Released {
    /// Which claim.
    pub claim: ClaimId,
    /// Keys that are now unclaimed.
    pub keys: Vec<KeyId>,
}

impl ClaimTable {
    /// Create a claim. `keys` must be non-empty and distinct (validated at
    /// the socket boundary).
    pub fn claim(&mut self, _owner: ConnId, _keys: &[KeyId], _color: Rgb) -> ClaimOutcome {
        todo!("layer 3")
    }

    /// Drop a claim. `None` if it does not exist or `owner` does not own it.
    pub fn release(&mut self, _owner: ConnId, _claim: ClaimId) -> Option<Released> {
        todo!("layer 3")
    }

    /// Drop every claim of a connection (disconnect, crash).
    pub fn drop_conn(&mut self, _owner: ConnId) -> Vec<Released> {
        todo!("layer 3")
    }

    /// Who should hear about a press of `key`.
    #[must_use]
    pub fn route(&self, _key: KeyId) -> Option<(ConnId, ClaimId)> {
        todo!("layer 3")
    }

    /// Colour per key for the current claims.
    #[must_use]
    pub fn frame(&self) -> crate::Frame {
        todo!("layer 3")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(i: u8) -> KeyId {
        KeyId::new(i).unwrap()
    }
    const RED: Rgb = Rgb { r: 255, g: 0, b: 0 };
    const BLUE: Rgb = Rgb { r: 0, g: 0, b: 255 };

    #[test]
    #[ignore = "layer 3 design"]
    fn newest_claim_wins_and_old_owner_is_told() {
        let mut t = ClaimTable::default();
        let a = t.claim(ConnId(1), &[k(0), k(1)], RED);
        let b = t.claim(ConnId(2), &[k(1)], BLUE);
        assert_eq!(b.preempted.len(), 1);
        assert_eq!(b.preempted[0].claim, a.id);
        assert_eq!(b.preempted[0].remaining, [k(0)]);
        assert_eq!(t.route(k(1)), Some((ConnId(2), b.id)));
    }

    #[test]
    #[ignore = "layer 3 design"]
    fn disconnect_releases_everything_of_that_connection() {
        let mut t = ClaimTable::default();
        t.claim(ConnId(1), &[k(0)], RED);
        t.claim(ConnId(1), &[k(5)], RED);
        assert_eq!(t.drop_conn(ConnId(1)).len(), 2);
        assert_eq!(t.route(k(0)), None);
        assert!(t.frame().is_dark());
    }

    #[test]
    #[ignore = "layer 3 design"]
    fn only_the_owner_can_release() {
        let mut t = ClaimTable::default();
        let a = t.claim(ConnId(1), &[k(0)], RED);
        assert_eq!(t.release(ConnId(2), a.id), None);
    }
}
