// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A property value from the wire into the model's [`Value`] — typed where
//! the tags say what it is, and always with the octets as sent.

use super::names::object_type_name;
use crate::object::value::Kind;
use crate::object::{
    BitString, CharacterString, Constructed, ConstructedElement, Date, ObjectId, Time, Value,
};
use regalium_bacnet_core::Error;
use regalium_bacnet_core::primitive::Primitive;
use regalium_bacnet_core::tag::{self, Class, Element};

/// The value a property's octets hold: one primitive when they are exactly
/// one application-tagged element, a [`Constructed`] otherwise.
///
/// [`Value::encoded`] is always the octets given, so a device that spelled
/// a length the long way, or sent an Unsigned above what an `int64` holds,
/// is still inspectable through the model; such a primitive has no `kind`.
/// Copies: the result owns everything and outlives `octets`.
///
/// # Errors
///
/// The tag layer's, when the octets are not well-formed elements.
pub fn value(octets: &[u8]) -> Result<Value, Error<'_>> {
    let all = tag::decode_all(octets)?;
    if let [one] = all.as_slice()
        && one.header.class == Class::Application
    {
        return Ok(Value {
            kind: Primitive::decode(one).ok().and_then(|p| kind(&p)),
            encoded: octets.to_vec(),
        });
    }
    let mut at = 0;
    Ok(Value {
        kind: Some(Kind::ConstructedValue(constructed(&all, &mut at))),
        encoded: octets.to_vec(),
    })
}

/// The elements from `at` up to the closing tag that ends this level, or
/// the end. `decode_all` has already checked that tags pair.
fn constructed(all: &[Element<'_>], at: &mut usize) -> Constructed {
    let mut elements = Vec::new();
    while let Some(e) = all.get(*at) {
        *at += 1;
        if e.is_closing() {
            break;
        }
        let context_tag = (e.header.class == Class::Context).then_some(i32::from(e.header.number));
        let mut own = Vec::new();
        e.encode(&mut own);
        let kind = if e.is_opening() {
            Some(Kind::ConstructedValue(constructed(all, at)))
        } else if e.header.class == Class::Context {
            // Only the property's grammar knows a context tag's type: carry
            // the contents rather than guess.
            Some(Kind::OctetStringValue(e.content.to_vec()))
        } else {
            Primitive::decode(e).ok().and_then(|p| kind(&p))
        };
        elements.push(ConstructedElement {
            context_tag,
            value: Some(Value { kind, encoded: own }),
        });
    }
    Constructed { elements }
}

/// A primitive as the model's oneof, or `None` when the model cannot hold
/// it — an Unsigned above `i64::MAX`, a character set it cannot convert.
fn kind(p: &Primitive<'_>) -> Option<Kind> {
    let unspecified = |v: u8| (v != 255).then_some(i32::from(v));
    Some(match p {
        Primitive::Null => Kind::NullValue(0),
        Primitive::Boolean(b) => Kind::BooleanValue(*b),
        Primitive::Unsigned(v) => Kind::UnsignedValue(i64::try_from(*v).ok()?),
        Primitive::Signed(v) => Kind::SignedValue(*v),
        Primitive::Real(v) => Kind::RealValue(*v),
        Primitive::Double(v) => Kind::DoubleValue(*v),
        Primitive::OctetString(o) => Kind::OctetStringValue(o.to_vec()),
        // The model numbers character sets from 1, wire octet 0 being UTF-8.
        Primitive::CharacterString(s) => Kind::CharacterStringValue(CharacterString {
            text: s.text()?.into_owned(),
            character_set: i32::from(s.charset) + 1,
        }),
        Primitive::BitString(b) => Kind::BitStringValue(BitString {
            bits: (0..b.len()).map(|i| b.get(i) == Some(true)).collect(),
        }),
        Primitive::Enumerated(v) => Kind::EnumeratedValue((*v).into()),
        Primitive::Date(d) => Kind::DateValue(Date {
            year: d.full_year().map(i32::from),
            month: unspecified(d.month),
            day: unspecified(d.day),
            day_of_week: unspecified(d.weekday),
        }),
        Primitive::Time(t) => Kind::TimeValue(Time {
            hour: unspecified(t.hour),
            minute: unspecified(t.minute),
            second: unspecified(t.second),
            hundredths: unspecified(t.hundredths),
        }),
        Primitive::ObjectIdentifier(id) => Kind::ObjectIdValue(ObjectId {
            object_type: object_type_name(id.object_type),
            instance: i32::try_from(id.instance).ok()?,
        }),
    })
}
