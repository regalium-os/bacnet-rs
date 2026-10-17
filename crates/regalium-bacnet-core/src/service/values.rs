// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A field's primitive as the type its grammar says it is — an instance that
//! must fit 32 bits, a property identifier, a flag — and the writer for a
//! constructed field.

use crate::error::{Error, ErrorKind};
use crate::primitive::{CharacterString, ObjectIdentifier, Primitive, tag};
use crate::tag::{Element, Header};

/// An Unsigned that must fit 32 bits: an instance, a property, a length.
pub(crate) fn unsigned<'a>(p: &Primitive<'a>) -> Result<u32, Error<'a>> {
    match p {
        Primitive::Unsigned(v) => u32::try_from(*v).map_err(|_| invalid(tag::UNSIGNED)),
        _ => Err(invalid(tag::UNSIGNED)),
    }
}

/// An Enumerated.
pub(crate) fn enumerated<'a>(p: &Primitive<'a>) -> Result<u32, Error<'a>> {
    match p {
        Primitive::Enumerated(v) => Ok(*v),
        _ => Err(invalid(tag::ENUMERATED)),
    }
}

/// A BOOLEAN.
pub(crate) fn boolean<'a>(p: &Primitive<'a>) -> Result<bool, Error<'a>> {
    match p {
        Primitive::Boolean(b) => Ok(*b),
        _ => Err(invalid(tag::BOOLEAN)),
    }
}

/// A REAL.
pub(crate) fn real<'a>(p: &Primitive<'a>) -> Result<f32, Error<'a>> {
    match p {
        Primitive::Real(v) => Ok(*v),
        _ => Err(invalid(tag::REAL)),
    }
}

/// A character string, borrowed.
pub(crate) fn text<'a>(p: &Primitive<'a>) -> Result<CharacterString<'a>, Error<'a>> {
    match p {
        Primitive::CharacterString(s) => Ok(*s),
        _ => Err(invalid(tag::CHARACTER_STRING)),
    }
}

/// Appends opening tag `n`, the octets, and closing tag `n`.
pub(crate) fn put_constructed(n: u8, inner: &[u8], out: &mut Vec<u8>) {
    Element {
        header: Header::opening(n),
        content: &[],
    }
    .encode(out);
    out.extend_from_slice(inner);
    Element {
        header: Header::closing(n),
        content: &[],
    }
    .encode(out);
}

/// An object identifier.
pub(crate) fn object<'a>(p: &Primitive<'a>) -> Result<ObjectIdentifier, Error<'a>> {
    match p {
        Primitive::ObjectIdentifier(id) => Ok(*id),
        _ => Err(invalid(tag::OBJECT_IDENTIFIER)),
    }
}

const fn invalid<'a>(tag: u8) -> Error<'a> {
    Error::new(ErrorKind::InvalidValue { tag }, &[], 0)
}
