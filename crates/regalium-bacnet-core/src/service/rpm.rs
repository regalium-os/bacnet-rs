// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `ReadPropertyMultiple` (§15.7): many properties of many objects in one
//! round trip, each answered with a value or an error of its own.
//!
//! One round trip rather than one per point is the difference between
//! polling a building every second and every minute. A property's failure
//! does not fail the others, which is why the answer carries an error per
//! property rather than one for the request.

use super::fields::Fields;
use super::reference::PropertyReference;
use super::service_error::ServiceError;
use super::values::{object, put_constructed};
use crate::error::Error;
use crate::primitive::{ObjectIdentifier, Primitive, tag};

/// The properties to read from one object.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ReadAccess {
    /// `[0]` the object.
    pub object: ObjectIdentifier,
    /// `[1]` its properties.
    pub properties: Vec<PropertyReference>,
}

/// A `ReadPropertyMultiple` request.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ReadPropertyMultiple {
    /// What to read, object by object.
    pub reads: Vec<ReadAccess>,
}

impl ReadPropertyMultiple {
    /// Decodes the service request. Copies; keeps no borrow.
    ///
    /// # Errors
    ///
    /// A missing field or unpaired tag anywhere in the list.
    pub fn decode(body: &[u8]) -> Result<Self, Error<'_>> {
        let reads = Fields::new(body).list(|f| {
            Ok(ReadAccess {
                object: object(&f.required(0, tag::OBJECT_IDENTIFIER)?)?,
                properties: Fields::new(f.constructed(1)?)
                    .list(|p| PropertyReference::read(p, 0))?,
            })
        })?;
        Ok(Self { reads })
    }

    /// Appends the service request.
    pub fn encode(&self, out: &mut Vec<u8>) {
        for r in &self.reads {
            Primitive::ObjectIdentifier(r.object).encode_context(0, out);
            let mut list = Vec::new();
            r.properties.iter().for_each(|p| p.write(0, &mut list));
            put_constructed(1, &list, out);
        }
    }
}

/// One property's answer: its value's octets, or why there is none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PropertyResult<'a> {
    /// `[2]`–`[3]` which property.
    pub property: PropertyReference,
    /// `[4]` the value, or `[5]` the error.
    pub result: Result<&'a [u8], ServiceError>,
}

/// One object's answers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ReadResult<'a> {
    /// `[0]` the object.
    pub object: ObjectIdentifier,
    /// `[1]` a result per property, in the order asked — or, for `all`, in
    /// the device's order.
    pub results: Vec<PropertyResult<'a>>,
}

/// A `ReadPropertyMultiple`-ACK. Borrows the APDU for every value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ReadPropertyMultipleAck<'a> {
    /// Object by object.
    pub objects: Vec<ReadResult<'a>>,
}

impl<'a> ReadPropertyMultipleAck<'a> {
    /// Decodes the service ACK.
    ///
    /// # Errors
    ///
    /// A missing field, a result that is neither `[4]` nor `[5]`, an
    /// unpaired tag.
    pub fn decode(body: &'a [u8]) -> Result<Self, Error<'a>> {
        let objects = Fields::new(body).list(|f| {
            let object_id = object(&f.required(0, tag::OBJECT_IDENTIFIER)?)?;
            let results = match f.optional_constructed(1)? {
                Some(list) => Fields::new(list).list(|r| {
                    let property = PropertyReference::read(r, 2)?;
                    let result = match r.optional_constructed(4)? {
                        Some(value) => Ok(value),
                        None => Err(ServiceError::decode(r.constructed(5)?)?),
                    };
                    Ok(PropertyResult { property, result })
                })?,
                None => Vec::new(),
            };
            Ok(ReadResult {
                object: object_id,
                results,
            })
        })?;
        Ok(Self { objects })
    }

    /// Appends the service ACK; values are written as held.
    pub fn encode(&self, out: &mut Vec<u8>) {
        for o in &self.objects {
            Primitive::ObjectIdentifier(o.object).encode_context(0, out);
            let mut list = Vec::new();
            for r in &o.results {
                r.property.write(2, &mut list);
                match r.result {
                    Ok(value) => put_constructed(4, value, &mut list),
                    Err(e) => {
                        let mut err = Vec::new();
                        e.encode(&mut err);
                        put_constructed(5, &err, &mut list);
                    }
                }
            }
            put_constructed(1, &list, out);
        }
    }
}
