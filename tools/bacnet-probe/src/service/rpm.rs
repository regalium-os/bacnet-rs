// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `ReadPropertyMultiple` (§15.7): many properties of many objects in one
//! round trip. On a device behind an MS/TP gateway, where each round trip
//! costs a token rotation, this is the difference between reading a
//! controller in seconds and in minutes.
//!
//! Each property succeeds or fails on its own: the ACK carries a value or an
//! error per property, so one unknown property does not lose the rest. The
//! grammar is the driver's; the values are the probe's.

use super::bridge::{id, object, octets, property, reference, values};
use super::{PropertyRef, PropertyResult};
use crate::tag::ObjectId;
use regalium_bacnet::service::{
    self as core, ReadAccess, ReadPropertyMultiple, ReadPropertyMultipleAck, ReadResult,
    ServiceError,
};

/// One object and the properties wanted from it (`ReadAccessSpecification`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    /// The object.
    pub object: ObjectId,
    /// What to read from it.
    pub properties: Vec<PropertyRef>,
}

/// What came back for one object (`ReadAccessResult`).
#[derive(Clone, Debug, PartialEq)]
pub struct Results {
    /// The object.
    pub object: ObjectId,
    /// Each property in request order, with its value or its error.
    pub properties: Vec<(PropertyRef, PropertyResult)>,
}

/// A `ReadPropertyMultiple` request body.
/// A `ReadPropertyMultiple` request body.
#[must_use]
pub fn request(specs: &[Spec]) -> Vec<u8> {
    let reads = specs
        .iter()
        .map(|s| ReadAccess {
            object: id(s.object),
            properties: s.properties.iter().map(|p| reference(*p)).collect(),
        })
        .collect();
    let mut out = Vec::new();
    ReadPropertyMultiple { reads }.encode(&mut out);
    out
}

/// Decodes a request body.
#[must_use]
pub fn decode_request(body: &[u8]) -> Option<Vec<Spec>> {
    let r = ReadPropertyMultiple::decode(body).ok()?;
    Some(
        r.reads
            .iter()
            .map(|a| Spec {
                object: object(a.object),
                properties: a.properties.iter().map(|p| property(*p)).collect(),
            })
            .collect(),
    )
}

/// A ReadPropertyMultiple-ACK body.
#[must_use]
pub fn ack(results: &[Results]) -> Vec<u8> {
    let encoded: Vec<Vec<Result<Vec<u8>, ServiceError>>> = results
        .iter()
        .map(|r| {
            r.properties
                .iter()
                .map(|(_, v)| match v {
                    Ok(values) => Ok(octets(values)),
                    Err((class, code)) => Err(ServiceError {
                        class: *class,
                        code: *code,
                    }),
                })
                .collect()
        })
        .collect();
    let objects = results
        .iter()
        .zip(&encoded)
        .map(|(r, values)| ReadResult {
            object: id(r.object),
            results: r
                .properties
                .iter()
                .zip(values)
                .map(|((p, _), v)| core::PropertyResult {
                    property: reference(*p),
                    result: v.as_ref().map(Vec::as_slice).map_err(|e| *e),
                })
                .collect(),
        })
        .collect();
    let mut out = Vec::new();
    ReadPropertyMultipleAck { objects }.encode(&mut out);
    out
}

/// Decodes an ACK body.
#[must_use]
pub fn decode_ack(body: &[u8]) -> Option<Vec<Results>> {
    let ack = ReadPropertyMultipleAck::decode(body).ok()?;
    ack.objects
        .iter()
        .map(|o| {
            let properties = o
                .results
                .iter()
                .map(|r| {
                    let value: PropertyResult = match r.result {
                        Ok(v) => Ok(values(v)?),
                        Err(e) => Err((e.class, e.code)),
                    };
                    Some((property(r.property), value))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Results {
                object: object(o.object),
                properties,
            })
        })
        .collect()
}
