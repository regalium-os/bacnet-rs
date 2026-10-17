// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The network addresses an NPDU carries when it crosses a router: a network
//! number and a MAC address on that network (§6.2.2).

/// The network number meaning every network, `DNET` X'FFFF' (§6.2.2).
pub const GLOBAL: u16 = 0xFFFF;

/// A network number and a MAC address on it: `DNET`/`DLEN`/`DADR` or
/// `SNET`/`SLEN`/`SADR`. Borrows the MAC octets from the decoded buffer.
///
/// The MAC is opaque here: six octets for BACnet/IP (address and port), one
/// for MS/TP, eighteen for BACnet/IPv6 and so on. Only the datalink that
/// owns the network knows how to read it, which is why the network layer
/// never does.
///
/// `Copy`, holding only a borrow: safe to share across threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Address<'a> {
    /// The network number. [`GLOBAL`] means every network.
    pub network: u16,
    /// The MAC address, its length the `DLEN`/`SLEN` octet. Empty is a
    /// broadcast on `network` — meaningful only for a destination.
    pub mac: &'a [u8],
}

impl Address<'_> {
    /// Whether this addresses every device on its network: a `DLEN` of 0.
    #[must_use]
    pub const fn is_broadcast(&self) -> bool {
        self.mac.is_empty()
    }

    /// Whether this addresses every network: `DNET` X'FFFF', which §6.2.2
    /// requires to carry a `DLEN` of 0.
    #[must_use]
    pub const fn is_global(&self) -> bool {
        self.network == GLOBAL
    }

    /// Writes `NET LEN ADR`. A MAC longer than 255 octets — which decoding
    /// cannot produce — is written as its first 255, so the length octet
    /// and the octets that follow it always agree.
    pub(crate) fn encode(&self, out: &mut Vec<u8>) {
        let len = u8::try_from(self.mac.len()).unwrap_or(u8::MAX);
        out.extend_from_slice(&self.network.to_be_bytes());
        out.push(len);
        out.extend_from_slice(self.mac.get(..usize::from(len)).unwrap_or(self.mac));
    }
}

/// Where a routed NPDU is going, and how many more routers may forward it.
///
/// The hop count travels only with a destination (§6.2.2), so it lives
/// here: a hop count without a destination cannot be represented.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Destination<'a> {
    /// The destination network and MAC.
    pub address: Address<'a>,
    /// Decremented by every router, which discards the message at 0.
    ///
    /// Per §6.2.3 a hop count rather than a time to live, because a
    /// `BACnet` internetwork has no shared clock to measure a time against:
    /// a count is the only bound on a routing loop a router can enforce
    /// alone. Originators set 255.
    pub hop_count: u8,
}
