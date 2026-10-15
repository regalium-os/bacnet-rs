// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! ASHRAE 135 §20.2 tags: application and context tags, extended tag numbers,
//! extended lengths, and opening/closing tags. Decoding copies into `Value`.

/// An object identifier: a 10-bit type and a 22-bit instance (§20.2.14).
///
/// `Default` is `analog-input:0`, the all-zero identifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ObjectId {
    /// `BACnetObjectType`, e.g. 0 analog-input, 8 device.
    pub kind: u16,
    /// Instance within the type; 22 bits, so at most 4 194 303.
    pub instance: u32,
}

impl ObjectId {
    /// An identifier from its two parts.
    #[must_use]
    pub const fn new(kind: u16, instance: u32) -> Self {
        Self { kind, instance }
    }

    /// Splits the 32-bit wire form: type in the top 10 bits.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // 32 - 22 = 10 bits: fits u16.
    pub fn from_raw(raw: u32) -> Self {
        Self {
            kind: (raw >> 22) as u16,
            instance: raw & 0x3F_FFFF,
        }
    }
    /// The 32-bit wire form.
    #[must_use]
    pub fn raw(self) -> u32 {
        (u32::from(self.kind) << 22) | (self.instance & 0x3F_FFFF)
    }
}

/// One decoded application-tagged value (§20.2.2–§20.2.14).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// Application tag 0.
    Null,
    /// Tag 1; the value lives in the length field.
    Bool(bool),
    /// Tag 2, up to 8 octets.
    Unsigned(u64),
    /// Tag 3, two's complement, up to 8 octets.
    Signed(i64),
    /// Tag 4, IEEE-754 single.
    Real(f32),
    /// Tag 5, IEEE-754 double.
    Double(f64),
    /// Tag 6, and any primitive this probe does not interpret.
    Octets(Vec<u8>),
    /// Tag 7, decoded from its character set to UTF-8.
    Text(String),
    /// Tag 8, most significant bit first.
    Bits(Vec<bool>),
    /// Tag 9, up to 4 octets.
    Enumerated(u32),
    /// Tag 10: year − 1900, month, day, weekday; 255 is "unspecified".
    Date([u8; 4]),
    /// Tag 11: hour, minute, second, hundredths; 255 is "unspecified".
    Time([u8; 4]),
    /// Tag 12.
    Object(ObjectId),
    /// A context-tagged item (primitive or constructed) the probe skipped.
    Context(u8),
}

/// A decoded tag header. `lvt` is the raw length/value/type field.
#[derive(Clone, Copy, Debug)]
pub struct Header {
    /// Tag number: the application type, or the context parameter number.
    pub number: u8,
    /// Context-specific (class bit set) rather than application.
    pub context: bool,
    /// The raw length/value/type field.
    pub lvt: u8,
    /// Octets of data following the header.
    pub len: usize,
    /// A context opening tag `[n] {`.
    pub opening: bool,
    /// A context closing tag `}`.
    pub closing: bool,
}

mod decode;
mod encode;

pub(crate) use decode::{be, text};
pub use decode::{decode, decode_context_unsigned, header};
pub use encode::{push_closing, push_header, push_object, push_opening, push_unsigned, push_value};
