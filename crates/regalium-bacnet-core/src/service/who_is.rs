// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Who-Is (§16.10): "which devices are out there?", optionally only those
//! whose instance falls in a range.

use super::fields::Fields;
use super::values::unsigned;
use crate::error::Error;
use crate::primitive::{Primitive, tag};

/// A Who-Is request. Without a range every device answers, which on a
/// large site is hundreds of I-Ams at once; a range is how a client asks
/// politely.
///
/// `Copy` plain data: safe to share and send. `Default` asks everyone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WhoIs {
    /// The lowest and highest instance to answer, inclusive (§16.10.1.1).
    /// The grammar allows both or neither, never one.
    pub range: Option<(u32, u32)>,
}

impl WhoIs {
    /// Decodes the service request. Copies the two numbers; keeps no borrow.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::MissingField`](crate::ErrorKind::MissingField) for a
    /// low limit without a high one; [`ErrorKind::Trailing`](crate::ErrorKind::Trailing)
    /// for anything after the range, including a high limit alone.
    pub fn decode(body: &[u8]) -> Result<Self, Error<'_>> {
        let mut f = Fields::new(body);
        let range = match f.optional(0, tag::UNSIGNED)? {
            None => None,
            Some(low) => Some((unsigned(&low)?, unsigned(&f.required(1, tag::UNSIGNED)?)?)),
        };
        f.end()?;
        Ok(Self { range })
    }

    /// Appends the service request, canonically.
    pub fn encode(&self, out: &mut Vec<u8>) {
        if let Some((low, high)) = self.range {
            Primitive::Unsigned(low.into()).encode_context(0, out);
            Primitive::Unsigned(high.into()).encode_context(1, out);
        }
    }

    /// Whether a device of this instance should answer (§16.10.2).
    #[must_use]
    pub const fn asks(&self, instance: u32) -> bool {
        match self.range {
            None => true,
            Some((low, high)) => low <= instance && instance <= high,
        }
    }
}
