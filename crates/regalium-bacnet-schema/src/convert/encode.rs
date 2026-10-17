// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A model [`Value`] back to octets, for a write: the octets it carries when
//! it has them — exactly what was read, round-tripped — and otherwise its
//! typed value, encoded canonically.

use crate::object::value::Kind;
use crate::object::{CharacterSet, Constructed, Value};
use regalium_bacnet_core::enumerated::parse_object_type;
use regalium_bacnet_core::primitive::{
    BitString, CharacterString, Date, ObjectIdentifier, Primitive, Time,
};
use regalium_bacnet_core::tag::{Element, Header};

/// The octets `value` stands for, or `None` when it has neither octets nor
/// a kind this build can write — a character set other than UTF-8, an
/// object type it cannot name, a context element without its octets.
#[must_use]
pub fn encode_value(value: &Value) -> Option<Vec<u8>> {
    if !value.encoded.is_empty() {
        return Some(value.encoded.clone());
    }
    let mut out = Vec::new();
    match value.kind.as_ref()? {
        Kind::ConstructedValue(c) => constructed(c, &mut out)?,
        kind => primitive(kind, &mut out)?,
    }
    Some(out)
}

fn constructed(c: &Constructed, out: &mut Vec<u8>) -> Option<()> {
    for e in &c.elements {
        let v = e.value.as_ref()?;
        match (e.context_tag, v.kind.as_ref()) {
            _ if !v.encoded.is_empty() => out.extend_from_slice(&v.encoded),
            (Some(n), Some(Kind::ConstructedValue(inner))) => {
                let n = u8::try_from(n).ok()?;
                Element {
                    header: Header::opening(n),
                    content: &[],
                }
                .encode(out);
                constructed(inner, out)?;
                Element {
                    header: Header::closing(n),
                    content: &[],
                }
                .encode(out);
            }
            // A context element's type is the property's to know: only its
            // raw contents can be written.
            (Some(n), Some(Kind::OctetStringValue(raw))) => {
                let len = u32::try_from(raw.len()).ok()?;
                Element {
                    header: Header::context(u8::try_from(n).ok()?, len),
                    content: raw,
                }
                .encode(out);
            }
            (None, Some(kind)) => primitive(kind, out)?,
            _ => return None,
        }
    }
    Some(())
}

fn field(v: Option<i32>, offset: i32) -> Option<u8> {
    v.map_or(Some(255), |v| u8::try_from(v - offset).ok())
}

fn primitive(kind: &Kind, out: &mut Vec<u8>) -> Option<()> {
    let bits: Vec<u8>;
    let p = match kind {
        Kind::NullValue(_) => Primitive::Null,
        Kind::BooleanValue(b) => Primitive::Boolean(*b),
        Kind::UnsignedValue(v) => Primitive::Unsigned(u64::try_from(*v).ok()?),
        Kind::SignedValue(v) => Primitive::Signed(*v),
        Kind::RealValue(v) => Primitive::Real(*v),
        Kind::DoubleValue(v) => Primitive::Double(*v),
        Kind::OctetStringValue(o) => Primitive::OctetString(o),
        Kind::CharacterStringValue(s) => {
            let utf8 = matches!(
                s.character_set(),
                CharacterSet::Utf8 | CharacterSet::Unspecified
            );
            if !utf8 {
                return None;
            }
            Primitive::CharacterString(CharacterString {
                charset: 0,
                bytes: s.text.as_bytes(),
            })
        }
        Kind::BitStringValue(b) => {
            bits = b
                .bits
                .chunks(8)
                .map(|c| {
                    c.iter()
                        .enumerate()
                        .fold(0u8, |acc, (i, on)| acc | (u8::from(*on) << (7 - i)))
                })
                .collect();
            let unused = u8::try_from((8 - b.bits.len() % 8) % 8).ok()?;
            Primitive::BitString(BitString {
                unused,
                bytes: &bits,
            })
        }
        Kind::EnumeratedValue(v) => Primitive::Enumerated(u32::try_from(*v).ok()?),
        Kind::DateValue(d) => Primitive::Date(Date {
            year: field(d.year, 1900)?,
            month: field(d.month, 0)?,
            day: field(d.day, 0)?,
            weekday: field(d.day_of_week, 0)?,
        }),
        Kind::TimeValue(t) => Primitive::Time(Time {
            hour: field(t.hour, 0)?,
            minute: field(t.minute, 0)?,
            second: field(t.second, 0)?,
            hundredths: field(t.hundredths, 0)?,
        }),
        Kind::ObjectIdValue(o) => Primitive::ObjectIdentifier(ObjectIdentifier {
            object_type: parse_object_type(&o.object_type)?,
            instance: u32::try_from(o.instance).ok()?,
        }),
        Kind::ConstructedValue(_) => return None,
    };
    p.encode(out);
    Some(())
}
