//! Who owns which key. Pure state machine: no I/O; time is passed in.
//!
//! Rules (ADR 0004):
//! - A claim is a set of keys plus a colour, owned by one connection.
//! - Newest claim wins: claiming a held key moves it to the new claim and the
//!   old owner gets a [`Preemption`]. Lost keys never return (no stacking).
//! - A claim with no keys left is gone.
//! - A claim may carry a deadline; [`ClaimTable::expire`] drops it after.
//! - Disconnect releases every claim of that connection.

use std::collections::BTreeMap;
use std::time::Instant;

use macropad_ipc::ClaimId;
use macropad_proto::{KEY_COUNT, KeyId, Rgb};

use crate::Frame;

/// Daemon-local connection handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnId(pub u64);

/// One live claim, as [`ClaimTable::claims`] reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// Owning connection.
    pub owner: ConnId,
    /// Colour of its keys.
    pub color: Rgb,
    /// Keys still held, ascending.
    pub keys: Vec<KeyId>,
    /// When it expires, if ever.
    pub deadline: Option<Instant>,
}

/// Every claim, plus the per-key owner index.
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

/// A claim removed by release, disconnect or expiry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Released {
    /// Which claim.
    pub claim: ClaimId,
    /// Its owner.
    pub owner: ConnId,
    /// Keys that are now unclaimed.
    pub keys: Vec<KeyId>,
}

impl ClaimTable {
    /// Create a claim. `keys` must be non-empty and distinct (validated at
    /// the socket boundary; duplicates are harmless here).
    pub fn claim(
        &mut self,
        owner: ConnId,
        keys: &[KeyId],
        color: Rgb,
        deadline: Option<Instant>,
    ) -> ClaimOutcome {
        self.next_id += 1;
        let id = ClaimId(self.next_id);
        let mut lost: BTreeMap<ClaimId, Vec<KeyId>> = BTreeMap::new();
        for &key in keys {
            let slot = &mut self.owner_of_key[usize::from(key.index())];
            if let Some(old) = slot.replace(id)
                && old != id
            {
                lost.entry(old).or_default().push(key);
            }
        }
        let mut preempted = Vec::new();
        for (old, lost_keys) in lost {
            let Some(claim) = self.claims.get_mut(&old) else {
                continue;
            };
            claim.keys.retain(|k| !lost_keys.contains(k));
            let remaining = claim.keys.clone();
            let owner = claim.owner;
            if remaining.is_empty() {
                self.claims.remove(&old);
            }
            preempted.push(Preemption {
                owner,
                claim: old,
                lost: lost_keys,
                remaining,
            });
        }
        let mut keys = keys.to_vec();
        keys.sort_unstable();
        keys.dedup();
        self.claims.insert(
            id,
            Claim {
                owner,
                color,
                keys,
                deadline,
            },
        );
        ClaimOutcome { id, preempted }
    }

    fn remove(&mut self, id: ClaimId) -> Option<Released> {
        let claim = self.claims.remove(&id)?;
        for &key in &claim.keys {
            self.owner_of_key[usize::from(key.index())] = None;
        }
        Some(Released {
            claim: id,
            owner: claim.owner,
            keys: claim.keys,
        })
    }

    /// Drop a claim. `None` if it does not exist or `owner` does not own it.
    pub fn release(&mut self, owner: ConnId, claim: ClaimId) -> Option<Released> {
        if self.claims.get(&claim)?.owner != owner {
            return None;
        }
        self.remove(claim)
    }

    /// Drop every claim of a connection (disconnect, crash).
    pub fn drop_conn(&mut self, owner: ConnId) -> Vec<Released> {
        let ids: Vec<_> = self
            .claims
            .iter()
            .filter(|(_, c)| c.owner == owner)
            .map(|(id, _)| *id)
            .collect();
        ids.into_iter().filter_map(|id| self.remove(id)).collect()
    }

    /// Drop every claim whose deadline is at or before `now`.
    pub fn expire(&mut self, now: Instant) -> Vec<Released> {
        let ids: Vec<_> = self
            .claims
            .iter()
            .filter(|(_, c)| c.deadline.is_some_and(|d| d <= now))
            .map(|(id, _)| *id)
            .collect();
        ids.into_iter().filter_map(|id| self.remove(id)).collect()
    }

    /// Earliest deadline of any claim.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        self.claims.values().filter_map(|c| c.deadline).min()
    }

    /// Who should hear about a press of `key`.
    #[must_use]
    pub fn route(&self, key: KeyId) -> Option<(ConnId, ClaimId)> {
        let id = self.owner_of_key[usize::from(key.index())]?;
        Some((self.claims.get(&id)?.owner, id))
    }

    /// All claims, oldest first.
    pub fn claims(&self) -> impl Iterator<Item = (ClaimId, &Claim)> {
        self.claims.iter().map(|(id, c)| (*id, c))
    }

    /// Number of claims `owner` holds.
    #[must_use]
    pub fn count_of(&self, owner: ConnId) -> usize {
        self.claims.values().filter(|c| c.owner == owner).count()
    }

    /// Colour per key for the current claims.
    #[must_use]
    pub fn frame(&self) -> Frame {
        let mut frame = Frame::default();
        for (i, id) in self.owner_of_key.iter().enumerate() {
            frame.0[i] = id.and_then(|id| self.claims.get(&id)).map(|c| c.color);
        }
        frame
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn k(i: u8) -> KeyId {
        KeyId::new(i).unwrap()
    }
    const RED: Rgb = Rgb { r: 255, g: 0, b: 0 };
    const BLUE: Rgb = Rgb { r: 0, g: 0, b: 255 };

    #[test]
    fn claim_lights_and_routes_its_keys() {
        let mut t = ClaimTable::default();
        let a = t.claim(ConnId(1), &[k(0), k(1)], RED, None);
        assert_eq!(a.preempted, []);
        assert_eq!(t.route(k(1)), Some((ConnId(1), a.id)));
        assert_eq!(t.route(k(2)), None);
        assert_eq!(t.frame().get(k(0)), Some(RED));
        assert_eq!(t.frame().get(k(2)), None);
    }

    #[test]
    fn newest_claim_wins_and_old_owner_is_told() {
        let mut t = ClaimTable::default();
        let a = t.claim(ConnId(1), &[k(0), k(1)], RED, None);
        let b = t.claim(ConnId(2), &[k(1)], BLUE, None);
        assert_eq!(
            b.preempted,
            [Preemption {
                owner: ConnId(1),
                claim: a.id,
                lost: vec![k(1)],
                remaining: vec![k(0)],
            }]
        );
        assert_eq!(t.route(k(1)), Some((ConnId(2), b.id)));
        assert_eq!(t.frame().get(k(1)), Some(BLUE));
    }

    #[test]
    fn fully_preempted_claim_is_gone_and_keys_do_not_come_back() {
        let mut t = ClaimTable::default();
        let a = t.claim(ConnId(1), &[k(0)], RED, None);
        let b = t.claim(ConnId(2), &[k(0)], BLUE, None);
        assert_eq!(b.preempted[0].remaining, []);
        assert_eq!(t.release(ConnId(1), a.id), None);
        t.release(ConnId(2), b.id).unwrap();
        assert_eq!(t.route(k(0)), None, "no stacking");
        assert!(t.frame().is_dark());
    }

    #[test]
    fn disconnect_releases_everything_of_that_connection() {
        let mut t = ClaimTable::default();
        t.claim(ConnId(1), &[k(0)], RED, None);
        t.claim(ConnId(1), &[k(5)], RED, None);
        let c = t.claim(ConnId(2), &[k(9)], BLUE, None);
        assert_eq!(t.drop_conn(ConnId(1)).len(), 2);
        assert_eq!(t.route(k(0)), None);
        assert_eq!(t.route(k(9)), Some((ConnId(2), c.id)));
    }

    #[test]
    fn only_the_owner_can_release() {
        let mut t = ClaimTable::default();
        let a = t.claim(ConnId(1), &[k(0)], RED, None);
        assert_eq!(t.release(ConnId(2), a.id), None);
        assert_eq!(t.release(ConnId(1), a.id).unwrap().keys, [k(0)]);
    }

    #[test]
    fn expiry_drops_due_claims_only() {
        let now = Instant::now();
        let mut t = ClaimTable::default();
        let a = t.claim(ConnId(1), &[k(0)], RED, Some(now + Duration::from_secs(1)));
        let b = t.claim(ConnId(1), &[k(1)], RED, Some(now + Duration::from_secs(5)));
        t.claim(ConnId(1), &[k(2)], RED, None);
        assert_eq!(t.next_deadline(), Some(now + Duration::from_secs(1)));
        assert_eq!(t.expire(now), []);
        let gone = t.expire(now + Duration::from_secs(2));
        assert_eq!(gone.iter().map(|r| r.claim).collect::<Vec<_>>(), [a.id]);
        assert_eq!(t.next_deadline(), Some(now + Duration::from_secs(5)));
        assert_eq!(t.route(k(1)).map(|r| r.1), Some(b.id));
    }
}
