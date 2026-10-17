// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The virtual MAC address (Annex U.5): three octets standing for an
//! eighteen-octet B/IPv6 address in NPDUs and routing tables.

use core::fmt;

/// A virtual MAC address. `Copy`, ordered and hashable: it keys a
/// transaction as an IPv4 address and port does on BACnet/IP.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Vmac(pub [u8; 3]);

impl Vmac {
    /// The device instance as a virtual MAC (Annex U.5): an instance is 22
    /// bits, so it fits three octets, and a device's own number is the
    /// natural address for it. A router port without a Device object needs
    /// one configured instead.
    #[must_use]
    pub const fn from_instance(instance: u32) -> Self {
        let [_, a, b, c] = (instance & 0x3F_FFFF).to_be_bytes();
        Self([a, b, c])
    }

    /// The octets as an NPDU's `DADR`/`SADR` carries them.
    #[must_use]
    pub const fn octets(self) -> [u8; 3] {
        self.0
    }

    /// Reads three octets, when there are exactly three.
    #[must_use]
    pub fn from_octets(octets: &[u8]) -> Option<Self> {
        <[u8; 3]>::try_from(octets).ok().map(Self)
    }
}

impl fmt::Display for Vmac {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c] = self.0;
        write!(f, "{a:02x}{b:02x}{c:02x}")
    }
}
