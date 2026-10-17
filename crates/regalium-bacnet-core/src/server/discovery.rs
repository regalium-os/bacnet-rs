// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Answering discovery: I-Am for a Who-Is that asks for this device, I-Have
//! for a Who-Has of an object it holds.

use super::Response;
use crate::enumerated::Segmentation;
use crate::obj::Database;
use crate::primitive::{CharacterString, Primitive};
use crate::service::{IAm, IHave, Sought, WhoHas, WhoIs, unconfirmed};
use crate::tag::{Element, elements};

/// The one primitive in a property's octets, when it is one.
fn primitive<'a>(octets: &'a [u8]) -> Option<Primitive<'a>> {
    let mut all = elements(octets);
    let first: Element<'a> = all.next()?.ok()?;
    Primitive::decode(&first).ok()
}

fn unsigned(db: &dyn Database, property: u32, default: u32) -> u32 {
    let device = db.device();
    db.read(device, property, None)
        .ok()
        .and_then(|v| match primitive(&v) {
            Some(Primitive::Unsigned(n)) => u32::try_from(n).ok(),
            _ => None,
        })
        .unwrap_or(default)
}

pub(super) fn who_is(body: &[u8], db: &dyn Database) -> Option<Response> {
    let device = db.device();
    if !WhoIs::decode(body).ok()?.asks(device.instance) {
        return None;
    }
    let segmentation = db
        .read(device, 107, None)
        .ok()
        .and_then(|v| match primitive(&v) {
            Some(Primitive::Enumerated(s)) => Some(Segmentation::from_u32(s)),
            _ => None,
        })
        .unwrap_or(Segmentation::NoSegmentation);
    let mut out = vec![0x10, unconfirmed::I_AM];
    IAm {
        device,
        max_apdu: unsigned(db, 62, 1476),
        segmentation,
        vendor_id: unsigned(db, 120, 0),
    }
    .encode(&mut out);
    Some(Response::Broadcast(out))
}

pub(super) fn who_has(body: &[u8], db: &dyn Database) -> Option<Response> {
    let w = WhoHas::decode(body).ok()?;
    let device = db.device();
    let asked = WhoIs { range: w.range };
    if !asked.asks(device.instance) {
        return None;
    }
    let names: Vec<_> = db
        .objects()
        .into_iter()
        .filter_map(|o| Some((o, db.read(o, 77, None).ok()?)))
        .collect();
    let (object, name) = names.iter().find(|(o, name)| match w.object {
        Sought::Identifier(id) => *o == id,
        Sought::Name(wanted) => match primitive(name) {
            Some(Primitive::CharacterString(s)) => s.text() == wanted.text(),
            _ => false,
        },
    })?;
    let Some(Primitive::CharacterString(name)) = primitive(name) else {
        return None;
    };
    let mut out = vec![0x10, unconfirmed::I_HAVE];
    let name = CharacterString {
        charset: name.charset,
        bytes: name.bytes,
    };
    IHave {
        device,
        object: *object,
        name,
    }
    .encode(&mut out);
    Some(Response::Broadcast(out))
}
