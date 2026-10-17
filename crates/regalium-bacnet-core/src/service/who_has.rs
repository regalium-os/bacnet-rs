// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Who-Has and I-Have (§16.9, §16.8): finding which device holds an object,
//! by identifier or by name, when the device is not known.

use super::fields::Fields;
use super::values::{object, text, unsigned};
use crate::error::{Error, ErrorKind};
use crate::primitive::{CharacterString, ObjectIdentifier, Primitive, tag};

/// What a Who-Has looks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sought<'a> {
    /// `[2]` an object identifier.
    Identifier(ObjectIdentifier),
    /// `[3]` an object name — how an operator knows a point.
    Name(CharacterString<'a>),
}

/// A Who-Has: which device holds this object? `Copy`; borrows the name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WhoHas<'a> {
    /// `[0]`–`[1]`: only devices in this instance range answer.
    pub range: Option<(u32, u32)>,
    /// The object.
    pub object: Sought<'a>,
}

impl<'a> WhoHas<'a> {
    /// Decodes the service request. Borrows `body` for a name.
    ///
    /// # Errors
    ///
    /// A half range, neither `[2]` nor `[3]`, or octets after.
    pub fn decode(body: &'a [u8]) -> Result<Self, Error<'a>> {
        let mut f = Fields::new(body);
        let range = match f.optional(0, tag::UNSIGNED)? {
            Some(low) => Some((unsigned(&low)?, unsigned(&f.required(1, tag::UNSIGNED)?)?)),
            None => None,
        };
        let object = if let Some(id) = f.optional(2, tag::OBJECT_IDENTIFIER)? {
            Sought::Identifier(object(&id)?)
        } else if let Some(name) = f.optional(3, tag::CHARACTER_STRING)? {
            Sought::Name(text(&name)?)
        } else {
            return Err(Error::new(ErrorKind::MissingField(2), body, 0));
        };
        f.end()?;
        Ok(Self { range, object })
    }

    /// Appends the service request, canonically.
    pub fn encode(&self, out: &mut Vec<u8>) {
        if let Some((low, high)) = self.range {
            Primitive::Unsigned(low.into()).encode_context(0, out);
            Primitive::Unsigned(high.into()).encode_context(1, out);
        }
        match self.object {
            Sought::Identifier(id) => Primitive::ObjectIdentifier(id).encode_context(2, out),
            Sought::Name(n) => Primitive::CharacterString(n).encode_context(3, out),
        }
    }
}

/// An I-Have: this device holds that object, by this name. `Copy`; borrows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IHave<'a> {
    /// The device that holds it.
    pub device: ObjectIdentifier,
    /// The object.
    pub object: ObjectIdentifier,
    /// Its object-name.
    pub name: CharacterString<'a>,
}

impl<'a> IHave<'a> {
    /// Decodes the service request: three application-tagged values.
    ///
    /// # Errors
    ///
    /// A missing or mistyped value, or octets after.
    pub fn decode(body: &'a [u8]) -> Result<Self, Error<'a>> {
        let mut f = Fields::new(body);
        let device = object(&f.application()?)?;
        let object_id = object(&f.application()?)?;
        let name = text(&f.application()?)?;
        f.end()?;
        Ok(Self {
            device,
            object: object_id,
            name,
        })
    }

    /// Appends the service request, canonically.
    pub fn encode(&self, out: &mut Vec<u8>) {
        Primitive::ObjectIdentifier(self.device).encode(out);
        Primitive::ObjectIdentifier(self.object).encode(out);
        Primitive::CharacterString(self.name).encode(out);
    }
}
