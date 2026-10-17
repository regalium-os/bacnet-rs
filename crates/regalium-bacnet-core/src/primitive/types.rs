// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The primitive types with structure: character strings, bit strings,
//! dates, times and object identifiers. Each keeps its octets as sent, so a
//! date with a month of 20 is held, and reported invalid, rather than
//! clamped into something the device did not say.

use std::borrow::Cow;

/// A character string (§20.2.9): a character-set octet and the encoded
/// text. Borrows the input; [`CharacterString::text`] decodes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CharacterString<'a> {
    /// 0 UTF-8 (ANSI X3.4), 1 DBCS, 2 JIS X 0208, 3 UCS-4, 4 UCS-2,
    /// 5 ISO 8859-1.
    pub charset: u8,
    /// The text in that set.
    pub bytes: &'a [u8],
}

impl CharacterString<'_> {
    /// The text as UTF-8: borrowed when it already is, decoded from UCS-2
    /// or ISO 8859-1, `None` for a set this crate does not decode or text
    /// that is not valid in its set.
    #[must_use]
    pub fn text(&self) -> Option<Cow<'_, str>> {
        match self.charset {
            0 => core::str::from_utf8(self.bytes).ok().map(Cow::Borrowed),
            4 => {
                let (units, rest) = self.bytes.as_chunks::<2>();
                if !rest.is_empty() {
                    return None;
                }
                let units: Vec<u16> = units.iter().map(|u| u16::from_be_bytes(*u)).collect();
                String::from_utf16(&units).ok().map(Cow::Owned)
            }
            5 => Some(Cow::Owned(
                self.bytes.iter().map(|b| char::from(*b)).collect(),
            )),
            _ => None,
        }
    }
}

/// A bit string (§20.2.10): how many bits of the last octet are unused,
/// then the octets, most significant bit first. Borrows the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BitString<'a> {
    /// Unused bits at the end of the last octet, 0 to 7.
    pub unused: u8,
    /// The bits.
    pub bytes: &'a [u8],
}

impl BitString<'_> {
    /// How many bits it carries.
    #[must_use]
    pub fn len(&self) -> usize {
        (self.bytes.len() * 8).saturating_sub(self.unused.into())
    }

    /// Whether it carries none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Bit `i`, counting from the first; `None` past the end.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<bool> {
        (i < self.len()).then(|| {
            self.bytes
                .get(i / 8)
                .is_some_and(|b| b & (0x80 >> (i % 8)) != 0)
        })
    }
}

/// A date (§20.2.12), as its four octets. 255 in any field is "unspecified".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Date {
    /// Year minus 1900.
    pub year: u8,
    /// 1 to 12; 13 odd months, 14 even months.
    pub month: u8,
    /// 1 to 31; 32 last day, 33 odd days, 34 even days.
    pub day: u8,
    /// 1 Monday to 7 Sunday.
    pub weekday: u8,
}

impl Date {
    /// The year, or `None` when unspecified.
    #[must_use]
    pub fn full_year(&self) -> Option<u16> {
        (self.year != 255).then(|| 1900 + u16::from(self.year))
    }

    /// Whether every field is in its range or unspecified.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let ok = |v: u8, lo: u8, hi: u8| v == 255 || (lo..=hi).contains(&v);
        ok(self.month, 1, 14) && ok(self.day, 1, 34) && ok(self.weekday, 1, 7)
    }
}

/// A time of day (§20.2.13), as its four octets. 255 is "unspecified".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Time {
    /// 0 to 23.
    pub hour: u8,
    /// 0 to 59.
    pub minute: u8,
    /// 0 to 59.
    pub second: u8,
    /// 0 to 99.
    pub hundredths: u8,
}

impl Time {
    /// Whether every field is in its range or unspecified.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let ok = |v: u8, hi: u8| v == 255 || v <= hi;
        ok(self.hour, 23) && ok(self.minute, 59) && ok(self.second, 59) && ok(self.hundredths, 99)
    }
}

/// An object identifier (§20.2.14): a 10-bit type and a 22-bit instance in
/// one four-octet field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectIdentifier {
    /// `BACnetObjectType`; 128 and above are proprietary.
    pub object_type: u16,
    /// 0 to 4 194 303.
    pub instance: u32,
}

impl ObjectIdentifier {
    /// The instance that means "this device" in a request (§12.11.1).
    pub const WILDCARD_INSTANCE: u32 = 0x3F_FFFF;

    /// Splits the wire form: type in the top ten bits.
    #[must_use]
    pub const fn from_raw(raw: u32) -> Self {
        Self {
            object_type: (raw >> 22) as u16,
            instance: raw & 0x3F_FFFF,
        }
    }

    /// The wire form. A type above 1023 or an instance above the wildcard
    /// is truncated to its field, as the field width requires.
    #[must_use]
    pub const fn raw(self) -> u32 {
        ((self.object_type as u32 & 0x3FF) << 22) | (self.instance & 0x3F_FFFF)
    }
}
