// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Values between the probe's codec and the schema: what was read goes out
//! as a schema `Value`, and what a caller writes comes in from one.
//!
//! Two things are lost on the way out, and said so rather than faked. The
//! probe's codec decodes text to UTF-8 and does not keep the octets a device
//! sent, so `character_set` is left unspecified and `encoded` empty; and a
//! constructed value the codec skipped has no kind at all. Both arrive when
//! the probe moves onto the core's lossless codec.

use super::error::ApiError;
use super::name;
use crate::{ObjectId, Value, names};
use regalium_bacnet_schema::object::{self as schema, value::Kind};

/// One value read from a device.
#[must_use]
pub fn to_schema(v: &Value) -> schema::Value {
    let kind = match v {
        Value::Null => Some(Kind::NullValue(prost_types::NullValue::NullValue.into())),
        Value::Bool(b) => Some(Kind::BooleanValue(*b)),
        // Above 2^63 - 1 saturates; such an unsigned has never been seen in a
        // building, and the lossless codec will carry it in `encoded`.
        Value::Unsigned(u) => Some(Kind::UnsignedValue(i64::try_from(*u).unwrap_or(i64::MAX))),
        Value::Signed(i) => Some(Kind::SignedValue(*i)),
        Value::Real(f) => Some(Kind::RealValue(*f)),
        Value::Double(d) => Some(Kind::DoubleValue(*d)),
        Value::Octets(o) => Some(Kind::OctetStringValue(o.clone())),
        Value::Text(t) => Some(Kind::CharacterStringValue(schema::CharacterString {
            text: t.clone(),
            character_set: schema::CharacterSet::Unspecified.into(),
        })),
        Value::Bits(b) => Some(Kind::BitStringValue(schema::BitString { bits: b.clone() })),
        Value::Enumerated(e) => Some(Kind::EnumeratedValue(i64::from(*e))),
        Value::Date([year, month, day, weekday]) => Some(Kind::DateValue(schema::Date {
            year: field(*year).map(|y| y + 1900),
            month: field(*month),
            day: field(*day),
            day_of_week: field(*weekday),
        })),
        Value::Time([hour, minute, second, hundredths]) => Some(Kind::TimeValue(schema::Time {
            hour: field(*hour),
            minute: field(*minute),
            second: field(*second),
            hundredths: field(*hundredths),
        })),
        Value::Object(id) => Some(Kind::ObjectIdValue(object_id(*id))),
        Value::Context(_) => None,
    };
    schema::Value {
        kind,
        encoded: Vec::new(),
    }
}

/// A property's values: one value as itself, several (an array, a list) as
/// a constructed value of their elements.
#[must_use]
pub fn values_to_schema(values: &[Value]) -> schema::Value {
    match values {
        [one] => to_schema(one),
        many => schema::Value {
            kind: Some(Kind::ConstructedValue(schema::Constructed {
                elements: many
                    .iter()
                    .map(|v| schema::ConstructedElement {
                        context_tag: None,
                        value: Some(to_schema(v)),
                    })
                    .collect(),
            })),
            encoded: Vec::new(),
        },
    }
}

/// An identifier as the schema spells it.
#[must_use]
pub(crate) fn object_id(id: ObjectId) -> schema::ObjectId {
    schema::ObjectId {
        object_type: names::object_type(id.kind),
        instance: i32::try_from(id.instance).unwrap_or(i32::MAX),
    }
}

/// What a caller asked to write, as values for the wire. A constructed value
/// becomes its elements, which is how an array or list is written whole.
///
/// # Errors
///
/// When the value is empty, out of its type's range, or a nested constructed
/// value, which the probe's codec does not encode.
pub fn from_schema(v: &schema::Value) -> Result<Vec<Value>, ApiError> {
    match &v.kind {
        Some(Kind::ConstructedValue(c)) => c
            .elements
            .iter()
            .map(|e| match e.value.as_ref().map(|v| &v.kind) {
                Some(Some(Kind::ConstructedValue(_))) | None => Err(ApiError::invalid(
                    "a nested constructed value cannot be written by the probe yet",
                )),
                Some(_) => one(e.value.as_ref().unwrap_or(v)),
            })
            .collect(),
        _ => one(v).map(|v| vec![v]),
    }
}

fn one(v: &schema::Value) -> Result<Value, ApiError> {
    let range = |what: &str| ApiError::invalid(format!("{what} is out of range"));
    Ok(
        match v
            .kind
            .as_ref()
            .ok_or_else(|| ApiError::invalid("a value with no kind"))?
        {
            Kind::NullValue(_) => Value::Null,
            Kind::BooleanValue(b) => Value::Bool(*b),
            Kind::UnsignedValue(u) => {
                Value::Unsigned(u64::try_from(*u).map_err(|_| range("unsigned"))?)
            }
            Kind::SignedValue(i) => Value::Signed(*i),
            Kind::RealValue(f) => Value::Real(*f),
            Kind::DoubleValue(d) => Value::Double(*d),
            Kind::OctetStringValue(o) => Value::Octets(o.clone()),
            Kind::CharacterStringValue(s) => match s.character_set() {
                schema::CharacterSet::Unspecified | schema::CharacterSet::Utf8 => {
                    Value::Text(s.text.clone())
                }
                other => {
                    return Err(ApiError::invalid(format!(
                        "the probe writes UTF-8 text only, not {}",
                        other.as_str_name()
                    )));
                }
            },
            Kind::BitStringValue(b) => Value::Bits(b.bits.clone()),
            Kind::EnumeratedValue(e) => {
                Value::Enumerated(u32::try_from(*e).map_err(|_| range("enumerated"))?)
            }
            Kind::DateValue(d) => Value::Date([
                octet(d.year.map(|y| y - 1900))?,
                octet(d.month)?,
                octet(d.day)?,
                octet(d.day_of_week)?,
            ]),
            Kind::TimeValue(t) => Value::Time([
                octet(t.hour)?,
                octet(t.minute)?,
                octet(t.second)?,
                octet(t.hundredths)?,
            ]),
            Kind::ObjectIdValue(id) => {
                let kind = name::parse_object_type(&id.object_type).ok_or_else(|| {
                    ApiError::invalid(format!("{:?} is not an object type", id.object_type))
                })?;
                let instance = u32::try_from(id.instance).map_err(|_| range("instance"))?;
                Value::Object(ObjectId::new(kind, instance))
            }
            Kind::ConstructedValue(_) => {
                return Err(ApiError::invalid(
                    "a nested constructed value cannot be written by the probe yet",
                ));
            }
        },
    )
}

/// A date or time octet: 255 is "unspecified" (§20.2.12).
fn field(octet: u8) -> Option<i32> {
    (octet != 255).then_some(i32::from(octet))
}

fn octet(field: Option<i32>) -> Result<u8, ApiError> {
    field.map_or(Ok(255), |f| {
        u8::try_from(f)
            .ok()
            .filter(|o| *o != 255)
            .ok_or_else(|| ApiError::invalid("a date or time field is out of range"))
    })
}
