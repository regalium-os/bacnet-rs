// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Writing a primitive, canonically: as few octets as the value needs.

use super::{Primitive, number, tag};
use crate::tag::Header;

impl Primitive<'_> {
    /// The application tag number of this type.
    #[must_use]
    pub fn tag(&self) -> u8 {
        match self {
            Self::Null => tag::NULL,
            Self::Boolean(_) => tag::BOOLEAN,
            Self::Unsigned(_) => tag::UNSIGNED,
            Self::Signed(_) => tag::SIGNED,
            Self::Real(_) => tag::REAL,
            Self::Double(_) => tag::DOUBLE,
            Self::OctetString(_) => tag::OCTET_STRING,
            Self::CharacterString(_) => tag::CHARACTER_STRING,
            Self::BitString(_) => tag::BIT_STRING,
            Self::Enumerated(_) => tag::ENUMERATED,
            Self::Date(_) => tag::DATE,
            Self::Time(_) => tag::TIME,
            Self::ObjectIdentifier(_) => tag::OBJECT_IDENTIFIER,
        }
    }

    /// The contents, canonically: as few octets as the value needs. Empty
    /// for an application BOOLEAN, which [`Primitive::encode`] puts in the
    /// header instead.
    #[must_use]
    pub fn content(&self) -> Vec<u8> {
        match *self {
            Self::Null | Self::Boolean(_) => Vec::new(),
            Self::Unsigned(v) => number::unsigned_octets(v),
            Self::Signed(v) => number::signed_octets(v),
            Self::Real(v) => v.to_be_bytes().to_vec(),
            Self::Double(v) => v.to_be_bytes().to_vec(),
            Self::OctetString(o) => o.to_vec(),
            Self::CharacterString(s) => [&[s.charset][..], s.bytes].concat(),
            Self::BitString(b) => [&[b.unused][..], b.bytes].concat(),
            Self::Enumerated(v) => number::unsigned_octets(v.into()),
            Self::Date(d) => vec![d.year, d.month, d.day, d.weekday],
            Self::Time(t) => vec![t.hour, t.minute, t.second, t.hundredths],
            Self::ObjectIdentifier(id) => id.raw().to_be_bytes().to_vec(),
        }
    }

    /// Appends the value application-tagged, canonically.
    pub fn encode(&self, out: &mut Vec<u8>) {
        if let Self::Boolean(b) = self {
            return Header::boolean(*b).encode(out);
        }
        let content = self.content();
        Header::application(self.tag(), u32::try_from(content.len()).unwrap_or(u32::MAX))
            .encode(out);
        out.extend(content);
    }

    /// Appends the value context-tagged with `number`, canonically. A
    /// BOOLEAN becomes one octet of contents here (§20.2.3).
    pub fn encode_context(&self, number: u8, out: &mut Vec<u8>) {
        let content = match self {
            Self::Boolean(b) => vec![u8::from(*b)],
            other => other.content(),
        };
        Header::context(number, u32::try_from(content.len()).unwrap_or(u32::MAX)).encode(out);
        out.extend(content);
    }
}
