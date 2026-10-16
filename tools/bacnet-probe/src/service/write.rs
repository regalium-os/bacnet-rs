// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `WriteProperty` (§15.9) and `WritePropertyMultiple` (§15.10).
//!
//! Both answer with a Simple-ACK. A value is a list of application-tagged
//! values, which covers every property a client normally writes —
//! present-value, out-of-service, object-name — and a NULL written at a
//! priority relinquishes that slot of a commandable object (§19.2.3).

use super::{PropertyRef, Reader};
use crate::tag::{self, ObjectId, Value};

/// One `BACnetPropertyValue` (§21): a property, its new value, and the
/// priority to write it at. `WritePropertyMultiple` and `CreateObject` both
/// carry lists of these.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyValue {
    /// The property, and optionally one element of it.
    pub property: PropertyRef,
    /// The value, as application-tagged values.
    pub values: Vec<Value>,
    /// 1 (highest) to 16, for a commandable property; `None` otherwise.
    pub priority: Option<u8>,
}

impl PropertyValue {
    /// A whole property, written without a priority.
    #[must_use]
    pub fn new(property: u32, values: Vec<Value>) -> Self {
        Self {
            property: PropertyRef::whole(property),
            values,
            priority: None,
        }
    }
}

/// A `WriteProperty` request body.
#[must_use]
pub fn request(object: ObjectId, write: &PropertyValue) -> Vec<u8> {
    let mut out = Vec::new();
    tag::push_object(&mut out, 0, true, object);
    tag::push_unsigned(&mut out, 1, true, write.property.property.into());
    if let Some(index) = write.property.index {
        tag::push_unsigned(&mut out, 2, true, index.into());
    }
    push_values(&mut out, 3, &write.values);
    if let Some(p) = write.priority {
        tag::push_unsigned(&mut out, 4, true, p.into());
    }
    out
}

/// Decodes a `WriteProperty` request body, borrowing it.
#[must_use]
pub fn decode_request(body: &[u8]) -> Option<(ObjectId, PropertyValue)> {
    let mut r = Reader::new(body);
    let object = r.object(0)?;
    let property = u32::try_from(r.unsigned(1)?).ok()?;
    let index = r.optional_unsigned(2)?.and_then(|i| u32::try_from(i).ok());
    r.opening(3)?;
    let values = r.values_until(3)?;
    let priority = r.optional_unsigned(4)?.and_then(|p| u8::try_from(p).ok());
    r.is_empty().then_some(())?;
    Some((
        object,
        PropertyValue {
            property: PropertyRef { property, index },
            values,
            priority,
        },
    ))
}

/// A `WritePropertyMultiple` request body: for each object, its writes.
#[must_use]
pub fn request_multiple(writes: &[(ObjectId, Vec<PropertyValue>)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (object, values) in writes {
        tag::push_object(&mut out, 0, true, *object);
        tag::push_opening(&mut out, 1);
        push_property_values(&mut out, values);
        tag::push_closing(&mut out, 1);
    }
    out
}

/// Decodes a `WritePropertyMultiple` request body, borrowing it.
#[must_use]
pub fn decode_request_multiple(body: &[u8]) -> Option<Vec<(ObjectId, Vec<PropertyValue>)>> {
    let mut r = Reader::new(body);
    let mut out = Vec::new();
    while !r.is_empty() {
        let object = r.object(0)?;
        r.opening(1)?;
        out.push((object, decode_property_values(&mut r, 1)?));
    }
    Some(out)
}

/// A list of `BACnetPropertyValue`s, each `[0] id [1] index [2] { value } [3] priority`.
pub(crate) fn push_property_values(out: &mut Vec<u8>, values: &[PropertyValue]) {
    for v in values {
        tag::push_unsigned(out, 0, true, v.property.property.into());
        if let Some(index) = v.property.index {
            tag::push_unsigned(out, 1, true, index.into());
        }
        push_values(out, 2, &v.values);
        if let Some(p) = v.priority {
            tag::push_unsigned(out, 3, true, p.into());
        }
    }
}

/// `BACnetPropertyValue`s up to and including closing tag `end`.
pub(crate) fn decode_property_values(r: &mut Reader<'_>, end: u8) -> Option<Vec<PropertyValue>> {
    let mut out = Vec::new();
    while !r.at_closing(end) {
        let property = u32::try_from(r.unsigned(0)?).ok()?;
        let index = r.optional_unsigned(1)?.and_then(|i| u32::try_from(i).ok());
        r.opening(2)?;
        let values = r.values_until(2)?;
        let priority = r.optional_unsigned(3)?.and_then(|p| u8::try_from(p).ok());
        out.push(PropertyValue {
            property: PropertyRef { property, index },
            values,
            priority,
        });
    }
    r.closing(end)?;
    Some(out)
}

fn push_values(out: &mut Vec<u8>, number: u8, values: &[Value]) {
    tag::push_opening(out, number);
    for v in values {
        tag::push_value(out, v);
    }
    tag::push_closing(out, number);
}
