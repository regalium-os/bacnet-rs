// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The thirteen primitive application types (§20.2.2–§20.2.14): what an
//! element's contents mean once its type is known.
//!
//! An application tag carries its type in its number; a context tag's type
//! is the service grammar's to say, so [`Primitive::decode_as`] takes it.
//! Decoding borrows: octet strings, character strings and bit strings point
//! into the input. Encoding is canonical — the fewest octets each value
//! needs — so a value from a device that spelled it long decodes, but
//! re-encodes short; the octets it actually sent are kept by the
//! [`Element`](crate::tag::Element) it came in.

mod decode;
mod encode;
mod number;
pub mod tag;
mod types;

pub use types::{BitString, CharacterString, Date, ObjectIdentifier, Time};

/// One primitive value. Borrows the input it was decoded from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Primitive<'a> {
    /// §20.2.2.
    Null,
    /// §20.2.3.
    Boolean(bool),
    /// §20.2.4: up to eight octets.
    Unsigned(u64),
    /// §20.2.5: up to eight octets, two's complement.
    Signed(i64),
    /// §20.2.6: IEEE 754 single precision.
    Real(f32),
    /// §20.2.7: IEEE 754 double precision.
    Double(f64),
    /// §20.2.8.
    OctetString(&'a [u8]),
    /// §20.2.9.
    CharacterString(CharacterString<'a>),
    /// §20.2.10.
    BitString(BitString<'a>),
    /// §20.2.11: up to four octets.
    Enumerated(u32),
    /// §20.2.12.
    Date(Date),
    /// §20.2.13.
    Time(Time),
    /// §20.2.14.
    ObjectIdentifier(ObjectIdentifier),
}
