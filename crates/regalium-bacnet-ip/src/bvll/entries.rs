// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! B/IP addresses and the two tables a BBMD keeps (Annex J.4.2, J.5.2): the
//! broadcast distribution table of peer BBMDs, and the foreign device table.

use std::net::{Ipv4Addr, SocketAddrV4};

/// Octets in a B/IP address: four of IPv4 address, two of UDP port. This is
/// also BACnet/IP's MAC address, as an NPDU carries it (Annex J.1.2).
pub const ADDRESS_LEN: usize = 6;

/// Reads a six-octet B/IP address.
#[must_use]
pub fn address(octets: &[u8]) -> Option<SocketAddrV4> {
    match octets {
        [a, b, c, d, hi, lo, ..] => Some(SocketAddrV4::new(
            Ipv4Addr::new(*a, *b, *c, *d),
            u16::from_be_bytes([*hi, *lo]),
        )),
        _ => None,
    }
}

/// Writes a six-octet B/IP address.
pub fn put_address(addr: SocketAddrV4, out: &mut Vec<u8>) {
    out.extend_from_slice(&addr.ip().octets());
    out.extend_from_slice(&addr.port().to_be_bytes());
}

/// One broadcast distribution table entry: a peer BBMD and its mask.
///
/// The mask says how the peer wants broadcasts delivered: all ones means
/// "send them to me and I will rebroadcast" (two-hop); a subnet mask means
/// "send a directed broadcast to my subnet" (one-hop), which only works
/// where routers pass directed broadcasts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BdtEntry {
    /// The peer BBMD.
    pub address: SocketAddrV4,
    /// Its broadcast distribution mask.
    pub mask: Ipv4Addr,
}

impl BdtEntry {
    /// Octets per entry.
    pub const LEN: usize = 10;

    /// Where a forwarded broadcast for this peer goes: the peer itself for
    /// an all-ones mask, else the directed broadcast of its subnet
    /// (Annex J.4.5).
    #[must_use]
    pub fn forward_to(&self) -> SocketAddrV4 {
        let ip = u32::from(*self.address.ip()) | !u32::from(self.mask);
        SocketAddrV4::new(Ipv4Addr::from(ip), self.address.port())
    }

    /// Appends the entry.
    pub fn encode(&self, out: &mut Vec<u8>) {
        put_address(self.address, out);
        out.extend_from_slice(&self.mask.octets());
    }
}

/// One foreign device table entry, as Read-FDT-Ack reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FdtEntry {
    /// The foreign device.
    pub address: SocketAddrV4,
    /// The time-to-live it registered with, in seconds.
    pub ttl: u16,
    /// Seconds before the BBMD drops it: the TTL plus 30 seconds of grace,
    /// counting down (Annex J.5.2.3).
    pub remaining: u16,
}

impl FdtEntry {
    /// Octets per entry.
    pub const LEN: usize = 10;

    /// Appends the entry.
    pub fn encode(&self, out: &mut Vec<u8>) {
        put_address(self.address, out);
        out.extend_from_slice(&self.ttl.to_be_bytes());
        out.extend_from_slice(&self.remaining.to_be_bytes());
    }
}

/// A broadcast distribution table on the wire, borrowed: a whole number of
/// [`BdtEntry`]s.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bdt<'a>(pub(crate) &'a [u8]);

/// A foreign device table on the wire, borrowed: a whole number of
/// [`FdtEntry`]s.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fdt<'a>(pub(crate) &'a [u8]);

impl<'a> Bdt<'a> {
    /// The table in `octets`, when they are a whole number of entries.
    #[must_use]
    pub fn new(octets: &'a [u8]) -> Option<Self> {
        octets
            .len()
            .is_multiple_of(BdtEntry::LEN)
            .then_some(Self(octets))
    }

    /// The octets.
    #[must_use]
    pub const fn octets(&self) -> &'a [u8] {
        self.0
    }

    /// The entries, in order.
    pub fn iter(&self) -> impl Iterator<Item = BdtEntry> + 'a {
        self.0
            .as_chunks::<{ BdtEntry::LEN }>()
            .0
            .iter()
            .filter_map(|e| {
                let address = address(e)?;
                let [a, b, c, d] = e.get(6..10)?.try_into().ok()?;
                Some(BdtEntry {
                    address,
                    mask: Ipv4Addr::new(a, b, c, d),
                })
            })
    }
}

impl<'a> Fdt<'a> {
    /// The table in `octets`, when they are a whole number of entries.
    #[must_use]
    pub fn new(octets: &'a [u8]) -> Option<Self> {
        octets
            .len()
            .is_multiple_of(FdtEntry::LEN)
            .then_some(Self(octets))
    }

    /// The octets.
    #[must_use]
    pub const fn octets(&self) -> &'a [u8] {
        self.0
    }

    /// The entries, in order.
    pub fn iter(&self) -> impl Iterator<Item = FdtEntry> + 'a {
        self.0
            .as_chunks::<{ FdtEntry::LEN }>()
            .0
            .iter()
            .filter_map(|e| {
                let [t0, t1, r0, r1] = e.get(6..10)?.try_into().ok()?;
                Some(FdtEntry {
                    address: address(e)?,
                    ttl: u16::from_be_bytes([t0, t1]),
                    remaining: u16::from_be_bytes([r0, r1]),
                })
            })
    }
}
