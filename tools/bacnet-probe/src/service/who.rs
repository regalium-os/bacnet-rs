// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Discovery (§16.9, §16.10): Who-Is / I-Am finds devices, Who-Has / I-Have
//! finds an object when you know its name but not which device holds it.
//!
//! The grammars are the driver's; these are the probe's shapes for them.

use super::bridge::{id, object};
use crate::tag::ObjectId;
use regalium_bacnet::enumerated::Segmentation;
use regalium_bacnet::primitive::CharacterString;
use regalium_bacnet::service::{self as core, Sought, WhoHas, WhoIs};

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

fn utf8(text: &str) -> CharacterString<'_> {
    CharacterString {
        charset: 0,
        bytes: text.as_bytes(),
    }
}

/// A Who-Is body, optionally limited to device instances `low..=high`.
#[must_use]
pub fn who_is(limits: Option<(u32, u32)>) -> Vec<u8> {
    let mut out = Vec::new();
    WhoIs { range: limits }.encode(&mut out);
    out
}

/// The limits of a Who-Is body; `Some(None)` is "everyone". A low limit
/// without a high one is malformed (§16.10.1.1), and `None`.
#[must_use]
pub fn decode_who_is(body: &[u8]) -> Option<Option<(u32, u32)>> {
    WhoIs::decode(body).ok().map(|w| w.range)
}

/// An I-Am body: all four parameters are application-tagged.
#[must_use]
pub fn i_am(me: &IAm) -> Vec<u8> {
    let mut out = Vec::new();
    core::IAm {
        device: id(me.device),
        max_apdu: u32::try_from(me.max_apdu).unwrap_or(u32::MAX),
        segmentation: Segmentation::from_u32(me.segmentation),
        vendor_id: u32::try_from(me.vendor).unwrap_or(u32::MAX),
    }
    .encode(&mut out);
    out
}

/// Decodes an I-Am body; `None` unless it names a Device object.
#[must_use]
pub fn decode_i_am(body: &[u8]) -> Option<IAm> {
    let i = core::IAm::decode(body).ok()?;
    (i.device.object_type == 8).then(|| IAm {
        device: object(i.device),
        max_apdu: i.max_apdu.into(),
        segmentation: i.segmentation.to_u32(),
        vendor: i.vendor_id.into(),
    })
}

/// A Who-Has body.
#[must_use]
pub fn who_has(limits: Option<(u32, u32)>, wanted: &Wanted) -> Vec<u8> {
    let object = match wanted {
        Wanted::Object(o) => Sought::Identifier(id(*o)),
        Wanted::Name(name) => Sought::Name(utf8(name)),
    };
    let mut out = Vec::new();
    WhoHas {
        range: limits,
        object,
    }
    .encode(&mut out);
    out
}

/// Decodes a Who-Has body.
#[must_use]
pub fn decode_who_has(body: &[u8]) -> Option<(Option<(u32, u32)>, Wanted)> {
    let w = WhoHas::decode(body).ok()?;
    let wanted = match w.object {
        Sought::Identifier(o) => Wanted::Object(object(o)),
        Sought::Name(n) => Wanted::Name(n.text()?.into_owned()),
    };
    Some((w.range, wanted))
}

/// An I-Have body.
#[must_use]
pub fn i_have(have: &IHave) -> Vec<u8> {
    let mut out = Vec::new();
    core::IHave {
        device: id(have.device),
        object: id(have.object),
        name: utf8(&have.name),
    }
    .encode(&mut out);
    out
}

/// Decodes an I-Have body.
#[must_use]
pub fn decode_i_have(body: &[u8]) -> Option<IHave> {
    let i = core::IHave::decode(body).ok()?;
    Some(IHave {
        device: object(i.device),
        object: object(i.object),
        name: i.name.text()?.into_owned(),
    })
}
