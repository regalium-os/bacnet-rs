// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Why a datagram is not a BVLL-IPv6 message, and which octets.

use core::fmt;

/// What was wrong. Non-exhaustive, so a kind can be added without breaking
/// a caller's `match`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Bvll6ErrorKind {
    /// Shorter than the header and the sender's virtual MAC.
    Truncated {
        /// Octets needed.
        needed: usize,
        /// Octets present.
        available: usize,
    },
    /// The type octet is not X'82'. X'81' is BACnet/IP's (Annex J).
    NotBacnetIpv6(u8),
    /// The fields after the virtual MAC are the wrong length for the function.
    WrongLength {
        /// The BVLC function.
        function: u8,
        /// Octets after the virtual MAC.
        len: usize,
    },
}

/// A decode failure. Borrows the datagram, so a caller logging it logs what
/// arrived.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bvll6Error<'a> {
    /// What was wrong.
    pub kind: Bvll6ErrorKind,
    /// The whole datagram.
    pub bytes: &'a [u8],
}

impl fmt::Display for Bvll6Error<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            Bvll6ErrorKind::Truncated { needed, available } => {
                write!(
                    f,
                    "BVLL-IPv6 truncated: {needed} octets needed, {available} present"
                )
            }
            Bvll6ErrorKind::NotBacnetIpv6(t) => write!(f, "BVLL type {t:#04x} is not BACnet/IPv6"),
            Bvll6ErrorKind::WrongLength { function, len } => {
                write!(
                    f,
                    "BVLC-IPv6 function {function:#04x} cannot carry {len} octets"
                )
            }
        }
    }
}

impl std::error::Error for Bvll6Error<'_> {}
