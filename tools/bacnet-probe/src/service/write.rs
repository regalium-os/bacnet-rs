// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `WriteProperty` (§15.9) and `WritePropertyMultiple` (§15.10).
//!
//! Both answer with a Simple-ACK. A value is a list of application-tagged
//! values, which covers every property a client normally writes —
//! present-value, out-of-service, object-name — and a NULL written at a
//! priority relinquishes that slot of a commandable object (§19.2.3).
//!
//! The grammar is the driver's; the values are the probe's.

use super::PropertyRef;
use super::bridge::{id, object, octets, property, reference, values};
use crate::tag::{ObjectId, Value};
use regalium_bacnet::service::{self as core, WriteAccess, WriteProperty, WritePropertyMultiple};

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
    let value = octets(&write.values);
    let mut out = Vec::new();
    WriteProperty {
        object: id(object),
        value: core_value(write, &value),
    }
    .encode(&mut out);
    out
}

/// Decodes a `WriteProperty` request body, borrowing it.
#[must_use]
pub fn decode_request(body: &[u8]) -> Option<(ObjectId, PropertyValue)> {
    let w = WriteProperty::decode(body).ok()?;
    Some((object(w.object), owned(&w.value)?))
}

/// A `WritePropertyMultiple` request body: for each object, its writes.
#[must_use]
pub fn request_multiple(writes: &[(ObjectId, Vec<PropertyValue>)]) -> Vec<u8> {
    let encoded: Vec<Vec<Vec<u8>>> = writes
        .iter()
        .map(|(_, values)| values.iter().map(|v| octets(&v.values)).collect())
        .collect();
    let writes = writes
        .iter()
        .zip(&encoded)
        .map(|((o, values), octets)| WriteAccess {
            object: id(*o),
            values: values
                .iter()
                .zip(octets)
                .map(|(v, o)| core_value(v, o))
                .collect(),
        })
        .collect();
    let mut out = Vec::new();
    WritePropertyMultiple { writes }.encode(&mut out);
    out
}

/// Decodes a `WritePropertyMultiple` request body, borrowing it.
#[must_use]
pub fn decode_request_multiple(body: &[u8]) -> Option<Vec<(ObjectId, Vec<PropertyValue>)>> {
    let w = WritePropertyMultiple::decode(body).ok()?;
    w.writes
        .iter()
        .map(|a| {
            Some((
                object(a.object),
                a.values.iter().map(owned).collect::<Option<Vec<_>>>()?,
            ))
        })
        .collect()
}

/// The driver's `BACnetPropertyValue` for `v`, its value already encoded
/// as `octets`.
pub(crate) fn core_value<'a>(v: &PropertyValue, octets: &'a [u8]) -> core::PropertyValue<'a> {
    core::PropertyValue {
        property: reference(v.property),
        value: octets,
        priority: v.priority,
    }
}

/// The probe's `PropertyValue` for a decoded one; `None` when its value
/// does not decode.
pub(crate) fn owned(v: &core::PropertyValue<'_>) -> Option<PropertyValue> {
    Some(PropertyValue {
        property: property(v.property),
        values: values(v.value)?,
        priority: v.priority,
    })
}
