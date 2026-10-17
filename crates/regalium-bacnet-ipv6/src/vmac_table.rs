// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The virtual MAC table (Annex U.5): which B/IPv6 address each virtual MAC
//! stands for, learnt from every frame and resolved when unknown — as a
//! state machine on a clock the caller supplies, so a resolution that times
//! out or answers twice is a table test.

use crate::bvll::{Bvll6, Function6, Vmac};
use core::time::Duration;
use std::collections::BTreeMap;
use std::net::SocketAddrV6;

/// Two addresses claiming one virtual MAC: a fault on the network,
/// reported, never resolved in favour of whichever answered last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Duplicate {
    /// The virtual MAC claimed twice.
    pub vmac: Vmac,
    /// The address the table holds for it, and keeps.
    pub known: SocketAddrV6,
    /// The other address claiming it.
    pub claimed: SocketAddrV6,
}

/// What [`VmacTable::resolve`] found.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Resolve {
    /// The address.
    Known(SocketAddrV6),
    /// Unknown: send this Address-Resolution to the multicast group.
    Ask(Vec<u8>),
    /// Already being asked for.
    Waiting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    address: SocketAddrV6,
    seen: Duration,
    fixed: bool,
}

/// The table.
///
/// `Default` timers: entries age out after 10 minutes unheard; a resolution
/// is asked three times, a second apart.
///
/// Plain data: `Send` and `Sync`. Owned by the link that sends for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VmacTable {
    own: Vmac,
    age: Duration,
    retry: Duration,
    attempts: u8,
    entries: BTreeMap<Vmac, Entry>,
    /// Being resolved: attempts made, and when the next is due.
    pending: BTreeMap<Vmac, (u8, Duration)>,
}

impl VmacTable {
    /// A table for the node whose virtual MAC is `own`.
    #[must_use]
    pub const fn new(own: Vmac) -> Self {
        Self {
            own,
            age: Duration::from_secs(600),
            retry: Duration::from_secs(1),
            attempts: 3,
            entries: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }

    /// How long a learnt entry lasts unheard, and how a resolution retries.
    #[must_use]
    pub const fn timers(mut self, age: Duration, retry: Duration, attempts: u8) -> Self {
        self.age = age;
        self.retry = retry;
        self.attempts = attempts;
        self
    }

    /// This node's virtual MAC.
    #[must_use]
    pub const fn own(&self) -> Vmac {
        self.own
    }

    /// A mapping known from configuration: it never ages out.
    pub fn insert(&mut self, vmac: Vmac, address: SocketAddrV6) {
        self.entries.insert(
            vmac,
            Entry {
                address,
                seen: Duration::ZERO,
                fixed: true,
            },
        );
        self.pending.remove(&vmac);
    }

    /// Learns that `vmac` is at `address`, from a frame received at `now`.
    /// A different address for a mapping still fresh is a duplicate.
    pub fn learn(&mut self, vmac: Vmac, address: SocketAddrV6, now: Duration) -> Option<Duplicate> {
        if vmac == self.own {
            return None;
        }
        self.pending.remove(&vmac);
        match self.entries.get_mut(&vmac) {
            Some(e) if e.address == address => e.seen = now,
            Some(e) if e.fixed || now.saturating_sub(e.seen) < self.age => {
                return Some(Duplicate {
                    vmac,
                    known: e.address,
                    claimed: address,
                });
            }
            _ => {
                self.entries.insert(
                    vmac,
                    Entry {
                        address,
                        seen: now,
                        fixed: false,
                    },
                );
            }
        }
        None
    }

    /// The address of `vmac`, if known and not aged out.
    #[must_use]
    pub fn lookup(&self, vmac: Vmac, now: Duration) -> Option<SocketAddrV6> {
        let e = self.entries.get(&vmac)?;
        (e.fixed || now.saturating_sub(e.seen) < self.age).then_some(e.address)
    }

    /// The address of `vmac`, or the Address-Resolution to ask for it.
    pub fn resolve(&mut self, vmac: Vmac, now: Duration) -> Resolve {
        if let Some(a) = self.lookup(vmac, now) {
            return Resolve::Known(a);
        }
        if self.pending.contains_key(&vmac) {
            return Resolve::Waiting;
        }
        self.pending
            .insert(vmac, (1, now.saturating_add(self.retry)));
        Resolve::Ask(self.ask(vmac))
    }

    fn ask(&self, vmac: Vmac) -> Vec<u8> {
        Bvll6::to_vec(self.own, Function6::AddressResolution(vmac))
    }

    /// Resolutions to ask again at `now`, and those given up on — whose
    /// waiting frames the link drops.
    pub fn poll(&mut self, now: Duration) -> (Vec<Vec<u8>>, Vec<Vmac>) {
        let due: Vec<Vmac> = self
            .pending
            .iter()
            .filter(|(_, (_, at))| *at <= now)
            .map(|(v, _)| *v)
            .collect();
        let (mut again, mut gone) = (Vec::new(), Vec::new());
        for vmac in due {
            let Some((tries, at)) = self.pending.get_mut(&vmac) else {
                continue;
            };
            if *tries >= self.attempts {
                self.pending.remove(&vmac);
                gone.push(vmac);
            } else {
                *tries += 1;
                *at = now.saturating_add(self.retry);
                again.push(self.ask(vmac));
            }
        }
        (again, gone)
    }

    /// When [`VmacTable::poll`] next has something to do.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        self.pending.values().map(|(_, at)| *at).min()
    }
}
