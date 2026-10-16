// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `CreateObject` (§15.3) and `DeleteObject` (§15.4), and the constructed
//! Error-PDU body that they and `WritePropertyMultiple` answer with.

use super::Reader;
use super::write::{self, PropertyValue};
use crate::tag::{self, ObjectId, Value};

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
    let mut out = Vec::new();
    tag::push_opening(&mut out, 0);
    match what {
        Specifier::Type(kind) => tag::push_unsigned(&mut out, 0, true, kind.into()),
        Specifier::Id(id) => tag::push_object(&mut out, 1, true, id),
    }
    tag::push_closing(&mut out, 0);
    if !initial.is_empty() {
        tag::push_opening(&mut out, 1);
        write::push_property_values(&mut out, initial);
        tag::push_closing(&mut out, 1);
    }
    out
}

/// Decodes a `CreateObject` request body, borrowing it.
#[must_use]
pub fn decode_create_request(body: &[u8]) -> Option<(Specifier, Vec<PropertyValue>)> {
    let mut r = Reader::new(body);
    r.opening(0)?;
    let what = if r.at_context(0) {
        Specifier::Type(u16::try_from(r.unsigned(0)?).ok()?)
    } else {
        Specifier::Id(r.object(1)?)
    };
    r.closing(0)?;
    let initial = if r.at_opening(1) {
        r.opening(1)?;
        write::decode_property_values(&mut r, 1)?
    } else {
        Vec::new()
    };
    r.is_empty().then_some((what, initial))
}

/// A `CreateObject` ACK, or a `DeleteObject` request: one application-tagged
/// object identifier.
#[must_use]
pub fn object_body(id: ObjectId) -> Vec<u8> {
    let mut out = Vec::new();
    tag::push_value(&mut out, &Value::Object(id));
    out
}

/// Decodes a body that is one application-tagged object identifier.
#[must_use]
pub fn decode_object_body(body: &[u8]) -> Option<ObjectId> {
    let mut r = Reader::new(body);
    match r.app()? {
        Value::Object(id) if r.is_empty() => Some(id),
        _ => None,
    }
}

/// An Error-PDU body whose error is wrapped, `[0] { class code }`, followed
/// by what failed: `CreateObject` says which initial value by number,
/// `WritePropertyMultiple` which object and property.
#[must_use]
pub fn constructed_error(class: u32, code: u32, failed: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    tag::push_opening(&mut out, 0);
    tag::push_value(&mut out, &Value::Enumerated(class));
    tag::push_value(&mut out, &Value::Enumerated(code));
    tag::push_closing(&mut out, 0);
    out.extend_from_slice(failed);
    out
}

/// The class and code of an Error-PDU body, plain (§18) or wrapped as
/// [`constructed_error`] writes it. What failed, after it, is not kept.
#[must_use]
pub fn decode_error(body: &[u8]) -> Option<(u32, u32)> {
    let mut r = Reader::new(body);
    let wrapped = r.at_opening(0);
    if wrapped {
        r.opening(0)?;
    }
    match (r.app()?, r.app()?) {
        (Value::Enumerated(class), Value::Enumerated(code)) => {
            if wrapped {
                r.closing(0)?;
            }
            Some((class, code))
        }
        _ => None,
    }
}
