// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `ReadProperty` (§15.5): one property of one object per round trip. The
//! grammar is the driver's; the values are the probe's.

use super::PropertyRef;
use super::bridge::{id, object, octets, property, reference, values};
use crate::tag::{ObjectId, Value};
use regalium_bacnet::service::{ReadProperty, ReadPropertyAck};

fn read(o: ObjectId, p: PropertyRef) -> ReadProperty {
    let r = reference(p);
    ReadProperty {
        object: id(o),
        property: r.property,
        index: r.index,
    }
}

/// A `ReadProperty` request body.
#[must_use]
pub fn request(object: ObjectId, property: PropertyRef) -> Vec<u8> {
    let mut out = Vec::new();
    read(object, property).encode(&mut out);
    out
}

/// Decodes a request body, borrowing it.
#[must_use]
pub fn decode_request(body: &[u8]) -> Option<(ObjectId, PropertyRef)> {
    let r = ReadProperty::decode(body).ok()?;
    let p = regalium_bacnet::service::PropertyReference {
        property: r.property,
        index: r.index,
    };
    Some((object(r.object), property(p)))
}

/// A ReadProperty-ACK body.
#[must_use]
pub fn ack(object: ObjectId, property: PropertyRef, values: &[Value]) -> Vec<u8> {
    let value = octets(values);
    let mut out = Vec::new();
    ReadPropertyAck {
        read: read(object, property),
        value: &value,
    }
    .encode(&mut out);
    out
}

/// The values in a ReadProperty-ACK body, borrowing it.
#[must_use]
pub fn decode_ack(body: &[u8]) -> Option<Vec<Value>> {
    values(ReadPropertyAck::decode(body).ok()?.value)
}
