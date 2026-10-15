// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `ReadProperty` (§15.5): one property of one object per round trip.

use super::{PropertyRef, Reader};
use crate::tag::{self, ObjectId, Value};

/// A `ReadProperty` request body.
#[must_use]
pub fn request(object: ObjectId, property: PropertyRef) -> Vec<u8> {
    let mut out = Vec::new();
    tag::push_object(&mut out, 0, true, object);
    tag::push_unsigned(&mut out, 1, true, property.property.into());
    if let Some(index) = property.index {
        tag::push_unsigned(&mut out, 2, true, index.into());
    }
    out
}

/// Decodes a request body, borrowing it.
#[must_use]
pub fn decode_request(body: &[u8]) -> Option<(ObjectId, PropertyRef)> {
    let mut r = Reader::new(body);
    let object = r.object(0)?;
    let property = u32::try_from(r.unsigned(1)?).ok()?;
    let index = r.optional_unsigned(2)?.and_then(|i| u32::try_from(i).ok());
    Some((object, PropertyRef { property, index }))
}

/// A ReadProperty-ACK body.
#[must_use]
pub fn ack(object: ObjectId, property: PropertyRef, values: &[Value]) -> Vec<u8> {
    let mut out = request(object, property);
    tag::push_opening(&mut out, 3);
    for v in values {
        tag::push_value(&mut out, v);
    }
    tag::push_closing(&mut out, 3);
    out
}

/// The values in a ReadProperty-ACK body, borrowing it.
#[must_use]
pub fn decode_ack(body: &[u8]) -> Option<Vec<Value>> {
    let mut r = Reader::new(body);
    r.object(0)?;
    r.unsigned(1)?;
    r.optional_unsigned(2)?;
    r.opening(3)?;
    r.values_until(3)
}
