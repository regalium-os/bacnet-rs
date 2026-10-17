// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `CreateObject` and `DeleteObject` (§15.3, §15.4): the object lifecycle.

use super::fields::Fields;
use super::reference::PropertyValue;
use super::service_error::ServiceError;
use super::values::{enumerated, object, put_constructed, unsigned};
use crate::error::{Error, ErrorKind};
use crate::primitive::{ObjectIdentifier, Primitive, tag};

/// What object to create: a type, for the device to choose the instance, or
/// a whole identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ObjectSpecifier {
    /// `[0]` the type alone.
    Type(u16),
    /// `[1]` type and instance.
    Identifier(ObjectIdentifier),
}

/// A `CreateObject` request. Borrows the initial values.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CreateObject<'a> {
    /// `[0]` what to create.
    pub object: ObjectSpecifier,
    /// `[1]` properties to set as it is made.
    pub values: Vec<PropertyValue<'a>>,
}

impl<'a> CreateObject<'a> {
    /// Decodes the service request.
    ///
    /// # Errors
    ///
    /// A specifier that is neither choice, an unpaired tag, octets after.
    pub fn decode(body: &'a [u8]) -> Result<Self, Error<'a>> {
        let mut f = Fields::new(body);
        let mut spec = Fields::new(f.constructed(0)?);
        let object_spec = if let Some(t) = spec.optional(0, tag::ENUMERATED)? {
            let t = enumerated(&t)?;
            ObjectSpecifier::Type(u16::try_from(t).map_err(|_| {
                Error::new(
                    ErrorKind::InvalidValue {
                        tag: tag::ENUMERATED,
                    },
                    body,
                    0,
                )
            })?)
        } else {
            ObjectSpecifier::Identifier(object(&spec.required(1, tag::OBJECT_IDENTIFIER)?)?)
        };
        spec.end()?;
        let values = match f.optional_constructed(1)? {
            Some(list) => PropertyValue::list(list)?,
            None => Vec::new(),
        };
        f.end()?;
        Ok(Self {
            object: object_spec,
            values,
        })
    }

    /// Appends the service request.
    pub fn encode(&self, out: &mut Vec<u8>) {
        let mut spec = Vec::new();
        match self.object {
            ObjectSpecifier::Type(t) => {
                Primitive::Enumerated(t.into()).encode_context(0, &mut spec);
            }
            ObjectSpecifier::Identifier(id) => {
                Primitive::ObjectIdentifier(id).encode_context(1, &mut spec);
            }
        }
        put_constructed(0, &spec, out);
        if !self.values.is_empty() {
            let mut values = Vec::new();
            self.values.iter().for_each(|v| v.write(&mut values));
            put_constructed(1, &values, out);
        }
    }
}

/// Why a `CreateObject` failed, and at which initial value (§15.3.1.3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CreateObjectError {
    /// `[0]` the error.
    pub error: ServiceError,
    /// `[1]` the initial value that failed, counting from 1; 0 when the
    /// failure was not in one.
    pub first_failed: u32,
}

impl CreateObjectError {
    /// Decodes the Error PDU's body.
    ///
    /// # Errors
    ///
    /// A missing or malformed `[0]` or `[1]`.
    pub fn decode(body: &[u8]) -> Result<Self, Error<'_>> {
        let mut f = Fields::new(body);
        let error = ServiceError::decode(f.constructed(0)?)?;
        let first_failed = unsigned(&f.required(1, tag::UNSIGNED)?)?;
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
        Primitive::Unsigned(self.first_failed.into()).encode_context(1, out);
    }
}

/// An object identifier alone, application-tagged: the body of a
/// `DeleteObject` request and of a `CreateObject`-ACK.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectBody(pub ObjectIdentifier);

impl ObjectBody {
    /// Decodes the body.
    ///
    /// # Errors
    ///
    /// Not an object identifier, or octets after it.
    pub fn decode(body: &[u8]) -> Result<Self, Error<'_>> {
        let mut f = Fields::new(body);
        let id = object(&f.application()?)?;
        f.end()?;
        Ok(Self(id))
    }

    /// Appends the body.
    pub fn encode(&self, out: &mut Vec<u8>) {
        Primitive::ObjectIdentifier(self.0).encode(out);
    }
}
