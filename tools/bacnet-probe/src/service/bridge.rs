// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Between the driver's service grammars and the probe's own shapes: a
//! value's octets as the probe's [`Value`]s, and identifiers and property
//! references as each side spells them.
//!
//! The grammars are the core's; what stays the probe's is how it shows a
//! value, which its tables, JSON and TUI are built on.

use super::PropertyRef;
use crate::tag::{self, ObjectId, Value};
use regalium_bacnet::primitive::ObjectIdentifier;
use regalium_bacnet::service::PropertyReference;

/// The values in a value's octets, one element at a time; a nested
/// constructed value is one `Value::Context`. `None` when one does not
/// decode.
pub(crate) fn values(mut octets: &[u8]) -> Option<Vec<Value>> {
    let mut all = Vec::new();
    while !octets.is_empty() {
        let (value, used) = tag::decode(octets)?;
        all.push(value);
        octets = octets.get(used..)?;
    }
    Some(all)
}

/// The octets of `values`, application-tagged.
pub(crate) fn octets(values: &[Value]) -> Vec<u8> {
    let mut out = Vec::new();
    for v in values {
        tag::push_value(&mut out, v);
    }
    out
}

pub(crate) const fn id(o: ObjectId) -> ObjectIdentifier {
    ObjectIdentifier {
        object_type: o.kind,
        instance: o.instance,
    }
}

pub(crate) const fn object(o: ObjectIdentifier) -> ObjectId {
    ObjectId::new(o.object_type, o.instance)
}

pub(crate) const fn reference(p: PropertyRef) -> PropertyReference {
    PropertyReference {
        property: p.property,
        index: p.index,
    }
}

pub(crate) const fn property(p: PropertyReference) -> PropertyRef {
    PropertyRef {
        property: p.property,
        index: p.index,
    }
}
