// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `ReadPropertyMultiple` (§15.7): many properties of many objects in one
//! round trip. On a device behind an MS/TP gateway, where each round trip
//! costs a token rotation, this is the difference between reading a
//! controller in seconds and in minutes.
//!
//! Each property succeeds or fails on its own: the ACK carries a value or an
//! error per property, so one unknown property does not lose the rest.

use super::{PropertyRef, PropertyResult, Reader};
use crate::tag::{self, ObjectId, Value};

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
#[must_use]
pub fn request(specs: &[Spec]) -> Vec<u8> {
    let mut out = Vec::new();
    for spec in specs {
        tag::push_object(&mut out, 0, true, spec.object);
        tag::push_opening(&mut out, 1);
        for p in &spec.properties {
            tag::push_unsigned(&mut out, 0, true, p.property.into());
            if let Some(i) = p.index {
                tag::push_unsigned(&mut out, 1, true, i.into());
            }
        }
        tag::push_closing(&mut out, 1);
    }
    out
}

/// Decodes a request body, borrowing it.
#[must_use]
pub fn decode_request(body: &[u8]) -> Option<Vec<Spec>> {
    let mut r = Reader::new(body);
    let mut specs = Vec::new();
    while !r.is_empty() {
        let object = r.object(0)?;
        r.opening(1)?;
        let mut properties = Vec::new();
        while !r.at_closing(1) {
            let property = u32::try_from(r.unsigned(0)?).ok()?;
            let index = r.optional_unsigned(1)?.and_then(|i| u32::try_from(i).ok());
            properties.push(PropertyRef { property, index });
        }
        r.closing(1)?;
        specs.push(Spec { object, properties });
    }
    Some(specs)
}

/// A ReadPropertyMultiple-ACK body.
#[must_use]
pub fn ack(results: &[Results]) -> Vec<u8> {
    let mut out = Vec::new();
    for result in results {
        tag::push_object(&mut out, 0, true, result.object);
        tag::push_opening(&mut out, 1);
        for (p, value) in &result.properties {
            tag::push_unsigned(&mut out, 2, true, p.property.into());
            if let Some(i) = p.index {
                tag::push_unsigned(&mut out, 3, true, i.into());
            }
            match value {
                Ok(values) => {
                    tag::push_opening(&mut out, 4);
                    for v in values {
                        tag::push_value(&mut out, v);
                    }
                    tag::push_closing(&mut out, 4);
                }
                Err((class, code)) => {
                    tag::push_opening(&mut out, 5);
                    tag::push_value(&mut out, &Value::Enumerated(*class));
                    tag::push_value(&mut out, &Value::Enumerated(*code));
                    tag::push_closing(&mut out, 5);
                }
            }
        }
        tag::push_closing(&mut out, 1);
    }
    out
}

/// Decodes an ACK body, borrowing it.
#[must_use]
pub fn decode_ack(body: &[u8]) -> Option<Vec<Results>> {
    let mut r = Reader::new(body);
    let mut results = Vec::new();
    while !r.is_empty() {
        let object = r.object(0)?;
        r.opening(1)?;
        let mut properties = Vec::new();
        while !r.at_closing(1) {
            let property = u32::try_from(r.unsigned(2)?).ok()?;
            let index = r.optional_unsigned(3)?.and_then(|i| u32::try_from(i).ok());
            let value = if r.at_opening(4) {
                r.opening(4)?;
                Ok(r.values_until(4)?)
            } else {
                r.opening(5)?;
                let (Value::Enumerated(class), Value::Enumerated(code)) = (r.app()?, r.app()?)
                else {
                    return None;
                };
                r.closing(5)?;
                Err((class, code))
            };
            properties.push((PropertyRef { property, index }, value));
        }
        r.closing(1)?;
        results.push(Results { object, properties });
    }
    Some(results)
}
