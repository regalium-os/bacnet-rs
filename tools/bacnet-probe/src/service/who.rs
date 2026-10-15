// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Discovery (§16.9, §16.10): Who-Is / I-Am finds devices, Who-Has / I-Have
//! finds an object when you know its name but not which device holds it.

use super::Reader;
use crate::tag::{self, ObjectId, Value};

/// I-Am: who a device is and what it can take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IAm {
    /// The device object.
    pub device: ObjectId,
    /// Largest APDU it accepts.
    pub max_apdu: u64,
    /// `BACnetSegmentation`: 0 both, 1 transmit, 2 receive, 3 none.
    pub segmentation: u32,
    /// ASHRAE vendor identifier.
    pub vendor: u64,
}

/// What a Who-Has looks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Wanted {
    /// An object by identifier.
    Object(ObjectId),
    /// An object by its `object-name`.
    Name(String),
}

/// I-Have: a device saying it holds the object asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IHave {
    /// The device that holds it.
    pub device: ObjectId,
    /// The object.
    pub object: ObjectId,
    /// Its `object-name`.
    pub name: String,
}

fn range(out: &mut Vec<u8>, range: Option<(u32, u32)>) {
    if let Some((low, high)) = range {
        tag::push_unsigned(out, 0, true, low.into());
        tag::push_unsigned(out, 1, true, high.into());
    }
}

#[allow(clippy::option_option)] // Outer: malformed; inner: no limits given.
fn decode_range(r: &mut Reader<'_>) -> Option<Option<(u32, u32)>> {
    let low = r.optional_unsigned(0)?;
    let high = r.optional_unsigned(1)?;
    Some(match (low, high) {
        (Some(l), Some(h)) => Some((u32::try_from(l).ok()?, u32::try_from(h).ok()?)),
        _ => None,
    })
}

/// A Who-Is body, optionally limited to device instances `low..=high`.
#[must_use]
pub fn who_is(limits: Option<(u32, u32)>) -> Vec<u8> {
    let mut out = Vec::new();
    range(&mut out, limits);
    out
}

/// The limits of a Who-Is body, borrowing it; `Some(None)` is "everyone".
#[must_use]
pub fn decode_who_is(body: &[u8]) -> Option<Option<(u32, u32)>> {
    decode_range(&mut Reader::new(body))
}

/// An I-Am body: all four parameters are application-tagged.
#[must_use]
pub fn i_am(me: &IAm) -> Vec<u8> {
    let mut out = Vec::new();
    for v in [
        Value::Object(me.device),
        Value::Unsigned(me.max_apdu),
        Value::Enumerated(me.segmentation),
        Value::Unsigned(me.vendor),
    ] {
        tag::push_value(&mut out, &v);
    }
    out
}

/// Decodes an I-Am body, borrowing it.
#[must_use]
pub fn decode_i_am(body: &[u8]) -> Option<IAm> {
    let mut r = Reader::new(body);
    let (Value::Object(device), Value::Unsigned(max_apdu)) = (r.app()?, r.app()?) else {
        return None;
    };
    let (Value::Enumerated(segmentation), Value::Unsigned(vendor)) = (r.app()?, r.app()?) else {
        return None;
    };
    (device.kind == 8).then_some(IAm {
        device,
        max_apdu,
        segmentation,
        vendor,
    })
}

/// A Who-Has body.
#[must_use]
pub fn who_has(limits: Option<(u32, u32)>, wanted: &Wanted) -> Vec<u8> {
    let mut out = Vec::new();
    range(&mut out, limits);
    match wanted {
        Wanted::Object(id) => tag::push_object(&mut out, 2, true, *id),
        Wanted::Name(name) => {
            tag::push_header(&mut out, 3, true, name.len() + 1);
            out.push(0); // Character set: ANSI X3.4 / UTF-8.
            out.extend(name.as_bytes());
        }
    }
    out
}

/// Decodes a Who-Has body, borrowing it.
#[must_use]
pub fn decode_who_has(body: &[u8]) -> Option<(Option<(u32, u32)>, Wanted)> {
    let mut r = Reader::new(body);
    let limits = decode_range(&mut r)?;
    let wanted = if r.at_context(2) {
        Wanted::Object(r.object(2)?)
    } else {
        Wanted::Name(r.text(3)?)
    };
    Some((limits, wanted))
}

/// An I-Have body.
#[must_use]
pub fn i_have(have: &IHave) -> Vec<u8> {
    let mut out = Vec::new();
    for v in [
        Value::Object(have.device),
        Value::Object(have.object),
        Value::Text(have.name.clone()),
    ] {
        tag::push_value(&mut out, &v);
    }
    out
}

/// Decodes an I-Have body, borrowing it.
#[must_use]
pub fn decode_i_have(body: &[u8]) -> Option<IHave> {
    let mut r = Reader::new(body);
    match (r.app()?, r.app()?, r.app()?) {
        (Value::Object(device), Value::Object(object), Value::Text(name)) => Some(IHave {
            device,
            object,
            name,
        }),
        _ => None,
    }
}
