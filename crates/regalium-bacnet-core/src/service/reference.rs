// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The references and values services share (§21): `BACnetPropertyReference`,
//! `BACnetObjectPropertyReference` and `BACnetPropertyValue`. Each is a
//! sequence of context tags whose numbers depend on where it sits, so the
//! readers take the first tag number.

use super::fields::Fields;
use super::values::{enumerated, object, put_constructed, unsigned};
use crate::error::Error;
use crate::primitive::{ObjectIdentifier, Primitive, tag};

/// A property, and optionally one element of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PropertyReference {
    /// `BACnetPropertyIdentifier`. 8 (`all`), 105 (`required`) and 80
    /// (`optional`) ask for many at once in `ReadPropertyMultiple`.
    pub property: u32,
    /// An array element, when only one is meant.
    pub index: Option<u32>,
}

impl PropertyReference {
    /// The whole of `property`.
    #[must_use]
    pub const fn new(property: u32) -> Self {
        Self {
            property,
            index: None,
        }
    }

    /// Reads `[n]` property and `[n + 1]` index.
    pub(crate) fn read<'a>(f: &mut Fields<'a>, n: u8) -> Result<Self, Error<'a>> {
        Ok(Self {
            property: enumerated(&f.required(n, tag::ENUMERATED)?)?,
            index: f
                .optional(n + 1, tag::UNSIGNED)?
                .map(|i| unsigned(&i))
                .transpose()?,
        })
    }

    pub(crate) fn write(&self, n: u8, out: &mut Vec<u8>) {
        Primitive::Enumerated(self.property).encode_context(n, out);
        if let Some(i) = self.index {
            Primitive::Unsigned(i.into()).encode_context(n + 1, out);
        }
    }
}

/// An object and one of its properties.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectPropertyReference {
    /// The object, `[0]`.
    pub object: ObjectIdentifier,
    /// The property, `[1]` and `[2]`.
    pub property: PropertyReference,
}

impl ObjectPropertyReference {
    pub(crate) fn read<'a>(f: &mut Fields<'a>) -> Result<Self, Error<'a>> {
        Ok(Self {
            object: object(&f.required(0, tag::OBJECT_IDENTIFIER)?)?,
            property: PropertyReference::read(f, 1)?,
        })
    }

    pub(crate) fn write(&self, out: &mut Vec<u8>) {
        Primitive::ObjectIdentifier(self.object).encode_context(0, out);
        self.property.write(1, out);
    }
}

/// A property's value, as written or as notified: `[0]` property, `[1]`
/// index, `[2]` the value, `[3]` priority.
///
/// The value is the octets between its tags, borrowed: only the property
/// knows its type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PropertyValue<'a> {
    /// Which property, and element.
    pub property: PropertyReference,
    /// The value's elements, undecoded.
    pub value: &'a [u8],
    /// The command priority, 1–16, for a commandable property.
    pub priority: Option<u8>,
}

impl<'a> PropertyValue<'a> {
    pub(crate) fn read(f: &mut Fields<'a>) -> Result<Self, Error<'a>> {
        let property = PropertyReference::read(f, 0)?;
        let value = f.constructed(2)?;
        let priority = f
            .optional(3, tag::UNSIGNED)?
            .map(|p| unsigned(&p))
            .transpose()?;
        Ok(Self {
            property,
            value,
            // A priority above 255 is no priority at all; keep what fits.
            priority: priority.map(|p| u8::try_from(p).unwrap_or(u8::MAX)),
        })
    }

    pub(crate) fn write(&self, out: &mut Vec<u8>) {
        self.property.write(0, out);
        put_constructed(2, self.value, out);
        if let Some(p) = self.priority {
            Primitive::Unsigned(p.into()).encode_context(3, out);
        }
    }

    /// Every value in a `SEQUENCE OF BACnetPropertyValue`.
    pub(crate) fn list(octets: &'a [u8]) -> Result<Vec<Self>, Error<'a>> {
        Fields::new(octets).list(Self::read)
    }
}
