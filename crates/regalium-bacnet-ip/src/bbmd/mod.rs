// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A `BACnet` Broadcast Management Device (Annex J.4, J.5), as a state machine
//! on a clock the caller supplies: given a message and the time, which
//! messages to send where, and what to deliver upward.
//!
//! A BBMD makes a broadcast on one subnet a broadcast on all of them. It
//! keeps a broadcast distribution table of peer BBMDs, one per subnet, and
//! a foreign device table of hosts registered with it from subnets that
//! have none; every broadcast it hears goes to both.

mod handle;

pub use handle::Handled;

use crate::bvll::{BdtEntry, Bvll, Function};
use core::time::Duration;
use std::net::SocketAddrV4;

/// The grace a BBMD adds to a foreign device's time-to-live before purging
/// it (Annex J.5.2.3), so a renewal a little late is not a lapse.
pub const GRACE: Duration = Duration::from_secs(30);

/// Where one outgoing message goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    /// One host: a peer BBMD, a foreign device, or a directed broadcast.
    Unicast(SocketAddrV4),
    /// The local subnet's broadcast address.
    LocalBroadcast,
}

/// One message to send.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Out {
    /// Where.
    pub to: Target,
    /// The BVLL message.
    pub bvll: Vec<u8>,
}

/// A registered foreign device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Foreign {
    /// Its address.
    pub address: SocketAddrV4,
    /// The time-to-live it registered with, seconds.
    pub ttl: u16,
    /// When it is purged: registration time, TTL and [`GRACE`].
    pub expires: Duration,
}

/// A BBMD's state: who it is, its peers, and its foreign devices.
///
/// Plain data: `Send` and `Sync`. Owned by the link that sends for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bbmd {
    me: SocketAddrV4,
    bdt: Vec<BdtEntry>,
    fdt: Vec<Foreign>,
    accept_foreign: bool,
    writable_bdt: bool,
}

impl Bbmd {
    /// The BBMD at `me` with this broadcast distribution table, which may
    /// list `me` itself — that entry's mask says how peers reach this
    /// subnet. Accepts foreign devices; refuses remote writes of its table.
    #[must_use]
    pub const fn new(me: SocketAddrV4, bdt: Vec<BdtEntry>) -> Self {
        Self {
            me,
            bdt,
            fdt: Vec::new(),
            accept_foreign: true,
            writable_bdt: false,
        }
    }

    /// Whether foreign devices may register.
    #[must_use]
    pub const fn foreign_devices(mut self, accept: bool) -> Self {
        self.accept_foreign = accept;
        self
    }

    /// Whether Write-Broadcast-Distribution-Table is obeyed. Off by default:
    /// a table anyone on the network can rewrite is a table anyone can use
    /// to send this subnet's broadcasts elsewhere.
    #[must_use]
    pub const fn writable_bdt(mut self, writable: bool) -> Self {
        self.writable_bdt = writable;
        self
    }

    /// The broadcast distribution table.
    #[must_use]
    pub fn bdt(&self) -> &[BdtEntry] {
        &self.bdt
    }

    /// The foreign device table.
    #[must_use]
    pub fn fdt(&self) -> &[Foreign] {
        &self.fdt
    }

    /// When the next foreign device lapses, if any is registered.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        self.fdt.iter().map(|f| f.expires).min()
    }

    /// Purges foreign devices whose registration lapsed by `now`.
    pub fn expire(&mut self, now: Duration) {
        self.fdt.retain(|f| f.expires > now);
    }

    /// This node's own broadcast of `npdu`: on the local subnet, and as a
    /// Forwarded-NPDU from this node to every peer and foreign device.
    #[must_use]
    pub fn broadcast(&self, npdu: &[u8]) -> Vec<Out> {
        let mut out = vec![Out {
            to: Target::LocalBroadcast,
            bvll: Bvll::to_vec(Function::OriginalBroadcast(npdu)),
        }];
        out.extend(self.relay(self.me, npdu, true, true, None));
        out
    }

    /// Forwarded-NPDUs of `npdu` from `origin`: to every peer when `peers`,
    /// to every foreign device when `foreign` except `except`.
    fn relay(
        &self,
        origin: SocketAddrV4,
        npdu: &[u8],
        peers: bool,
        foreign: bool,
        except: Option<SocketAddrV4>,
    ) -> Vec<Out> {
        let bvll = Bvll::to_vec(Function::Forwarded { origin, npdu });
        let to_peers = self
            .bdt
            .iter()
            .filter(|_| peers)
            .filter(|e| e.address != self.me)
            .map(BdtEntry::forward_to);
        let to_foreign = self
            .fdt
            .iter()
            .filter(|_| foreign)
            .map(|f| f.address)
            .filter(|a| Some(*a) != except);
        to_peers
            .chain(to_foreign)
            .map(|a| Out {
                to: Target::Unicast(a),
                bvll: bvll.clone(),
            })
            .collect()
    }

    /// Whether this node's own table entry asks peers for two-hop delivery,
    /// in which case it must rebroadcast what they forward.
    fn two_hop(&self) -> bool {
        self.bdt
            .iter()
            .any(|e| e.address == self.me && e.mask.is_broadcast())
    }
}
