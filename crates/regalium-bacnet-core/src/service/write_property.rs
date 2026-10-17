// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `WriteProperty` and `WritePropertyMultiple` (§15.9, §15.10).

use super::fields::Fields;
use super::reference::{ObjectPropertyReference, PropertyReference, PropertyValue};
use super::service_error::ServiceError;
use super::values::{object, put_constructed, unsigned};
use crate::error::Error;
use crate::primitive::{ObjectIdentifier, Primitive, tag};

/// A `WriteProperty`: one value into one property, at a priority when the
/// property is commandable.
///
/// The priority is why a write can "succeed" and change nothing: a write at
/// priority 16 is overruled by anything already held at 8 (§19.2). `Copy`;
/// borrows the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WriteProperty<'a> {
    /// `[0]` the object.
    pub object: ObjectIdentifier,
    /// `[1]`–`[3]` the property, element, value; `[4]` the priority.
    pub value: PropertyValue<'a>,
}

impl<'a> WriteProperty<'a> {
    /// Decodes the service request. Borrows `body` for the value.
    ///
    /// # Errors
    ///
    /// A missing field, an unpaired `[3]`, or octets after.
    pub fn decode(body: &'a [u8]) -> Result<Self, Error<'a>> {
        let mut f = Fields::new(body);
        let object_id = object(&f.required(0, tag::OBJECT_IDENTIFIER)?)?;
        let property = PropertyReference::read(&mut f, 1)?;
        let value = f.constructed(3)?;
        let priority = f
            .optional(4, tag::UNSIGNED)?
            .map(|p| unsigned(&p))
            .transpose()?;
        f.end()?;
        Ok(Self {
            object: object_id,
            value: PropertyValue {
                property,
                value,
                priority: priority.map(|p| u8::try_from(p).unwrap_or(u8::MAX)),
            },
        })
    }

    /// Appends the service request; the value is written as held.
    pub fn encode(&self, out: &mut Vec<u8>) {
        Primitive::ObjectIdentifier(self.object).encode_context(0, out);
        self.value.property.write(1, out);
        put_constructed(3, self.value.value, out);
        if let Some(p) = self.value.priority {
            Primitive::Unsigned(p.into()).encode_context(4, out);
        }
    }
}

/// The values for one object in a `WritePropertyMultiple`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WriteAccess<'a> {
    /// `[0]` the object.
    pub object: ObjectIdentifier,
    /// `[1]` its values.
    pub values: Vec<PropertyValue<'a>>,
}

/// A `WritePropertyMultiple`: many values, many objects, one request.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WritePropertyMultiple<'a> {
    /// The writes, in order; the device stops at the first that fails.
    pub writes: Vec<WriteAccess<'a>>,
}

impl<'a> WritePropertyMultiple<'a> {
    /// Decodes the service request. Borrows `body` for every value.
    ///
    /// # Errors
    ///
    /// A missing field or unpaired tag anywhere in the list.
    pub fn decode(body: &'a [u8]) -> Result<Self, Error<'a>> {
        let writes = Fields::new(body).list(|f| {
            Ok(WriteAccess {
                object: object(&f.required(0, tag::OBJECT_IDENTIFIER)?)?,
                values: PropertyValue::list(f.constructed(1)?)?,
            })
        })?;
        Ok(Self { writes })
    }

    /// Appends the service request.
    pub fn encode(&self, out: &mut Vec<u8>) {
        for w in &self.writes {
            Primitive::ObjectIdentifier(w.object).encode_context(0, out);
            let mut values = Vec::new();
            w.values.iter().for_each(|v| v.write(&mut values));
            put_constructed(1, &values, out);
        }
    }
}

/// Why a `WritePropertyMultiple` failed, and the first write that did (§15.10.1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WriteMultipleError {
    /// `[0]` the error.
    pub error: ServiceError,
    /// `[1]` the first write the device could not make; the ones before it
    /// were made.
    pub first_failed: ObjectPropertyReference,
}

impl WriteMultipleError {
    /// Decodes the Error PDU's body.
    ///
    /// # Errors
    ///
    /// A missing or malformed `[0]` or `[1]`.
    pub fn decode(body: &[u8]) -> Result<Self, Error<'_>> {
        let mut f = Fields::new(body);
        let error = ServiceError::decode(f.constructed(0)?)?;
        let first_failed = ObjectPropertyReference::read(&mut Fields::new(f.constructed(1)?))?;
        f.end()?;
        Ok(Self {
            error,
            first_failed,
        })
    }

    /// Appends the error.
    pub fn encode(&self, out: &mut Vec<u8>) {
        let mut e = Vec::new();
        self.error.encode(&mut e);
        put_constructed(0, &e, out);
        let mut r = Vec::new();
        self.first_failed.write(&mut r);
        put_constructed(1, &r, out);
    }
}
