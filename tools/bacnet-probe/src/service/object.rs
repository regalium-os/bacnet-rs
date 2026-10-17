// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `CreateObject` (§15.3) and `DeleteObject` (§15.4), and the constructed
//! Error-PDU body that they and `WritePropertyMultiple` answer with. The
//! grammars are the driver's; these are the probe's shapes for them.

use super::bridge::{id, object, octets};
use super::write::{PropertyValue, core_value, owned};
use crate::tag::{self, ObjectId};
use regalium_bacnet::service::{
    CreateObject, CreateObjectError, ObjectBody, ObjectSpecifier, ServiceError, WriteMultipleError,
};

/// What to create: a type, leaving the device to choose the instance, or an
/// exact identifier. Not every device can choose; one that cannot refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Specifier {
    /// `[0] objectType`: the device picks the instance.
    Type(u16),
    /// `[1] objectIdentifier`: exactly this object.
    Id(ObjectId),
}

/// A `CreateObject` request body.
#[must_use]
pub fn create_request(what: Specifier, initial: &[PropertyValue]) -> Vec<u8> {
    let encoded: Vec<Vec<u8>> = initial.iter().map(|v| octets(&v.values)).collect();
    let object = match what {
        Specifier::Type(kind) => ObjectSpecifier::Type(kind),
        Specifier::Id(o) => ObjectSpecifier::Identifier(id(o)),
    };
    let values = initial
        .iter()
        .zip(&encoded)
        .map(|(v, o)| core_value(v, o))
        .collect();
    let mut out = Vec::new();
    CreateObject { object, values }.encode(&mut out);
    out
}

/// Decodes a `CreateObject` request body, borrowing it.
#[must_use]
pub fn decode_create_request(body: &[u8]) -> Option<(Specifier, Vec<PropertyValue>)> {
    let c = CreateObject::decode(body).ok()?;
    let what = match c.object {
        ObjectSpecifier::Type(kind) => Specifier::Type(kind),
        ObjectSpecifier::Identifier(o) => Specifier::Id(object(o)),
    };
    Some((
        what,
        c.values.iter().map(owned).collect::<Option<Vec<_>>>()?,
    ))
}

/// A `CreateObject` ACK, or a `DeleteObject` request: one application-tagged
/// object identifier.
#[must_use]
pub fn object_body(o: ObjectId) -> Vec<u8> {
    let mut out = Vec::new();
    ObjectBody(id(o)).encode(&mut out);
    out
}

/// Decodes a body that is one application-tagged object identifier.
#[must_use]
pub fn decode_object_body(body: &[u8]) -> Option<ObjectId> {
    ObjectBody::decode(body).ok().map(|b| object(b.0))
}

/// An Error-PDU body whose error is wrapped, `[0] { class code }`, followed
/// by what failed: `CreateObject` says which initial value by number,
/// `WritePropertyMultiple` which object and property.
#[must_use]
pub fn constructed_error(class: u32, code: u32, failed: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    tag::push_opening(&mut out, 0);
    ServiceError { class, code }.encode(&mut out);
    tag::push_closing(&mut out, 0);
    out.extend_from_slice(failed);
    out
}

/// The class and code of an Error-PDU body: plain (§18), or wrapped as a
/// `CreateObject` or `WritePropertyMultiple` error. What failed is not kept.
#[must_use]
pub fn decode_error(body: &[u8]) -> Option<(u32, u32)> {
    let e = ServiceError::decode(body)
        .ok()
        .or_else(|| CreateObjectError::decode(body).ok().map(|e| e.error))
        .or_else(|| WriteMultipleError::decode(body).ok().map(|e| e.error))?;
    Some((e.class, e.code))
}
