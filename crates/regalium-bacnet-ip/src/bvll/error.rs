// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Why a datagram is not a BVLL message, and which octets.

use core::fmt;

/// What was wrong. Non-exhaustive, so a kind can be added without breaking
/// a caller's `match`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BvllErrorKind {
    /// Shorter than the four-octet header.
    Truncated {
        /// Octets needed.
        needed: usize,
        /// Octets present.
        available: usize,
    },
    /// The type octet is not X'81', BACnet/IP's (Annex J.2). X'82' is
    /// BACnet/IPv6's, and anything else on the port is not `BACnet` at all.
    NotBacnetIp(u8),
    /// The data is the wrong length for the function: a BVLC-Result that is
    /// not two octets, a table that is not a whole number of entries.
    WrongLength {
        /// The BVLC function.
        function: u8,
        /// Octets of data after the header.
        len: usize,
    },
}

/// A decode failure. Borrows the datagram, so a caller logging it logs what
/// arrived.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BvllError<'a> {
    /// What was wrong.
    pub kind: BvllErrorKind,
    /// The whole datagram.
    pub bytes: &'a [u8],
}

impl fmt::Display for BvllError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            BvllErrorKind::Truncated { needed, available } => {
                write!(
                    f,
                    "BVLL truncated: {needed} octets needed, {available} present"
                )
            }
            BvllErrorKind::NotBacnetIp(t) => write!(f, "BVLL type {t:#04x} is not BACnet/IP"),
            BvllErrorKind::WrongLength { function, len } => {
                write!(f, "BVLC function {function:#04x} cannot carry {len} octets")
            }
        }
    }
}

impl std::error::Error for BvllError<'_> {}
