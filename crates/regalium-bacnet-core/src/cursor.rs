// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A read position in a borrowed buffer, for the fixed-layout headers of the
//! NPDU and APDU: every read is checked, so a short buffer is an error
//! carrying the octets, never an index out of bounds.

use crate::error::{Error, ErrorKind};

/// Reads forward through `buf`. Borrows it; every slice returned is a
/// slice of it.
pub(crate) struct Cursor<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) const fn new(buf: &'a [u8]) -> Self {
        Self { buf, at: 0 }
    }

    /// The next `n` octets. A short buffer is [`ErrorKind::Truncated`],
    /// counted from the start of the PDU as the tag layer counts from the
    /// start of the element.
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], Error<'a>> {
        let end = self.at.saturating_add(n);
        let octets = self.buf.get(self.at..end).ok_or(Error::new(
            ErrorKind::Truncated {
                needed: end,
                available: self.buf.len(),
            },
            self.buf,
            0,
        ))?;
        self.at = end;
        Ok(octets)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, Error<'a>> {
        Ok(self.take(1)?.first().copied().unwrap_or(0))
    }

    /// A big-endian 16-bit number, as every multi-octet field of Clause 6
    /// and §20.1 is written.
    pub(crate) fn u16(&mut self) -> Result<u16, Error<'a>> {
        let o = self.take(2)?;
        Ok(o.iter().fold(0, |acc, b| (acc << 8) | u16::from(*b)))
    }

    /// Everything not yet read.
    pub(crate) fn rest(&mut self) -> &'a [u8] {
        let rest = self.buf.get(self.at..).unwrap_or(&[]);
        self.at = self.buf.len();
        rest
    }
}
