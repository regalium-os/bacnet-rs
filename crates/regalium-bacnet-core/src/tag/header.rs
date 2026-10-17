// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The tag header (§20.2.1): one initial octet — tag number in the high
//! nibble, class in bit 3, length/value/type in the low three bits — and the
//! octets its escapes call for.
//!
//! Two escapes exist, and both may be used when they are not needed: a tag
//! number of 15 in the nibble means the real number follows in an octet
//! (§20.2.1.2), and a length/value/type of 5 means the real length follows
//! in one, three or five octets (§20.2.1.3.1). A device that spells length 2
//! the long way is not wrong, only unusual, so the spelling is kept: a
//! header re-encodes to exactly the octets it was decoded from, and
//! [`Header::is_canonical`] says whether those were the shortest.

/// Application tags carry their type in the tag number; context tags carry
/// a parameter's position, and their type is the grammar's to know.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Bit 3 clear: the tag number is the datatype (§20.2.1.1).
    Application,
    /// Bit 3 set: the tag number is the field's context (§20.2.1.1).
    Context,
}

/// How a length was spelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Length {
    /// 0 to 4, in the length/value/type field itself (5 and above are not
    /// lengths there, so are never constructed here).
    Inline(u8),
    /// Escape 5, then one octet: 0 to 253 (254 and 255 are the next escapes,
    /// so a value of either here would not survive a round trip).
    Extended8(u8),
    /// Escape 5, then 254, then two octets.
    Extended16(u16),
    /// Escape 5, then 255, then four octets.
    Extended32(u32),
}

impl Length {
    /// The shortest spelling of `len`, as an encoder should write it.
    #[must_use]
    pub fn minimal(len: u32) -> Self {
        match len {
            0..=4 => Self::Inline(u8::try_from(len).unwrap_or(0)),
            5..=253 => Self::Extended8(u8::try_from(len).unwrap_or(0)),
            254..=0xFFFF => Self::Extended16(u16::try_from(len).unwrap_or(0)),
            _ => Self::Extended32(len),
        }
    }

    /// The length, whatever the spelling.
    #[must_use]
    pub fn value(self) -> u32 {
        match self {
            Self::Inline(n) | Self::Extended8(n) => n.into(),
            Self::Extended16(n) => n.into(),
            Self::Extended32(n) => n,
        }
    }
}

/// What the length/value/type field says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lvt {
    /// The contents' length.
    Length(Length),
    /// An application BOOLEAN carries its value here, with no contents
    /// (§20.2.3). Kept raw: a device sending 2 is malformed, and is shown so.
    Boolean(u8),
    /// A context opening tag, `[n] {` (§20.2.1.3.2).
    Opening,
    /// A context closing tag, `}` (§20.2.1.3.2).
    Closing,
}

/// One tag header, as it was spelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// Application or context.
    pub class: Class,
    /// The tag number, 0 to 254.
    pub number: u8,
    /// Whether the number was written after the escape nibble 15.
    pub extended_number: bool,
    /// The length/value/type field, interpreted.
    pub lvt: Lvt,
}

impl Header {
    /// An application tag for `len` octets of contents, spelled minimally.
    #[must_use]
    pub fn application(number: u8, len: u32) -> Self {
        Self {
            class: Class::Application,
            number,
            extended_number: number >= 15,
            lvt: Lvt::Length(Length::minimal(len)),
        }
    }

    /// A context tag for `len` octets of contents, spelled minimally.
    #[must_use]
    pub fn context(number: u8, len: u32) -> Self {
        Self {
            class: Class::Context,
            ..Self::application(number, len)
        }
    }

    /// A context opening tag, `[number] {`.
    #[must_use]
    pub fn opening(number: u8) -> Self {
        Self {
            lvt: Lvt::Opening,
            ..Self::context(number, 0)
        }
    }

    /// A context closing tag.
    #[must_use]
    pub fn closing(number: u8) -> Self {
        Self {
            lvt: Lvt::Closing,
            ..Self::context(number, 0)
        }
    }

    /// An application BOOLEAN.
    #[must_use]
    pub fn boolean(value: bool) -> Self {
        Self {
            lvt: Lvt::Boolean(value.into()),
            ..Self::application(1, 0)
        }
    }

    /// Octets of contents that follow the header.
    #[must_use]
    pub fn content_len(&self) -> u32 {
        match self.lvt {
            Lvt::Length(l) => l.value(),
            Lvt::Boolean(_) | Lvt::Opening | Lvt::Closing => 0,
        }
    }

    /// Whether this is the shortest spelling of what it says. A canonical
    /// encoder writes only these; a device need not.
    #[must_use]
    pub fn is_canonical(&self) -> bool {
        let number = self.extended_number == (self.number >= 15);
        match self.lvt {
            Lvt::Length(l) => number && l == Length::minimal(l.value()),
            Lvt::Boolean(v) => number && v <= 1,
            Lvt::Opening | Lvt::Closing => number,
        }
    }
}
