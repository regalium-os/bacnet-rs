// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `ReadProperty` (§15.5): one property of one object, and its answer.

use super::fields::{Fields, enumerated, object, unsigned};
use crate::error::Error;
use crate::primitive::{ObjectIdentifier, Primitive, tag};
use crate::tag::{Element, Elements, Header, elements};

/// A `ReadProperty` request.
///
/// `Copy` plain data: safe to share and send.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReadProperty {
    /// [0] The object.
    pub object: ObjectIdentifier,
    /// [1] `BACnetPropertyIdentifier`; see [`crate::enumerated::property`].
    pub property: u32,
    /// [2] One element of an array property; 0 asks for its length. Absent
    /// reads the whole property.
    pub index: Option<u32>,
}

/// The fields both the request and its answer open with.
fn reference<'a>(f: &mut Fields<'a>) -> Result<ReadProperty, Error<'a>> {
    Ok(ReadProperty {
        object: object(&f.required(0, tag::OBJECT_IDENTIFIER)?)?,
        property: enumerated(&f.required(1, tag::ENUMERATED)?)?,
        index: match f.optional(2, tag::UNSIGNED)? {
            Some(i) => Some(unsigned(&i)?),
            None => None,
        },
    })
}

impl ReadProperty {
    /// Decodes the service request. Copies; keeps no borrow.
    ///
    /// # Errors
    ///
    /// A missing [0] or [1], a mistyped field, or octets after them.
    pub fn decode(body: &[u8]) -> Result<Self, Error<'_>> {
        let mut f = Fields::new(body);
        let r = reference(&mut f)?;
        f.end()?;
        Ok(r)
    }

    /// Appends the service request, canonically.
    pub fn encode(&self, out: &mut Vec<u8>) {
        Primitive::ObjectIdentifier(self.object).encode_context(0, out);
        Primitive::Enumerated(self.property).encode_context(1, out);
        if let Some(i) = self.index {
            Primitive::Unsigned(i.into()).encode_context(2, out);
        }
    }
}

/// A `ReadProperty`-ACK: the request's reference and the value read.
///
/// The value is left as the octets between its opening and closing tags,
/// borrowed from the APDU: a property may be any type — a REAL, a priority
/// array, a list of structures — and only the property knows which.
/// [`ReadPropertyAck::values`] walks its elements; the octets are what a
/// lossless model keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReadPropertyAck<'a> {
    /// What was read: the request's [0]–[2], echoed.
    pub read: ReadProperty,
    /// [3] The value's elements, undecoded.
    pub value: &'a [u8],
}

impl<'a> ReadPropertyAck<'a> {
    /// Decodes the service ACK. Borrows `body` for the value.
    ///
    /// # Errors
    ///
    /// A missing field, an unpaired [3], or octets after it.
    pub fn decode(body: &'a [u8]) -> Result<Self, Error<'a>> {
        let mut f = Fields::new(body);
        let read = reference(&mut f)?;
        let value = f.constructed(3)?;
        f.end()?;
        Ok(Self { read, value })
    }

    /// Appends the service ACK; the value is written as held.
    pub fn encode(&self, out: &mut Vec<u8>) {
        self.read.encode(out);
        Element {
            header: Header::opening(3),
            content: &[],
        }
        .encode(out);
        out.extend_from_slice(self.value);
        Element {
            header: Header::closing(3),
            content: &[],
        }
        .encode(out);
    }

    /// The value's elements, in order.
    #[must_use]
    pub fn values(&self) -> Elements<'a> {
        elements(self.value)
    }

    /// The value as one primitive, when it is exactly one application-tagged
    /// element — as present-value, object-name and most properties are.
    #[must_use]
    pub fn primitive(&self) -> Option<Primitive<'a>> {
        let mut all = self.values();
        let first = all.next()?.ok()?;
        if all.next().is_some() {
            return None;
        }
        Primitive::decode(&first).ok()
    }
}
