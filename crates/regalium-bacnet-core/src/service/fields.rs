// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Reading a service body field by field, as its ASN.1 grammar orders them
//! (Clause 21): a context tag the grammar names, an optional one that may
//! be absent, an application-tagged value, a constructed value between an
//! opening and a closing tag.

use crate::error::{Error, ErrorKind};
use crate::primitive::{ObjectIdentifier, Primitive, tag};
use crate::tag::{Class, Element};

/// The marker [`ErrorKind::MissingField`] uses for an application tag.
const APPLICATION: u8 = 255;

/// A read position in a service body. Borrows it.
pub(crate) struct Fields<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Fields<'a> {
    pub(crate) const fn new(buf: &'a [u8]) -> Self {
        Self { buf, at: 0 }
    }

    fn rest(&self) -> &'a [u8] {
        self.buf.get(self.at..).unwrap_or(&[])
    }

    fn fail(&self, kind: ErrorKind) -> Error<'a> {
        Error::new(kind, self.rest(), self.at)
    }

    /// The next element without consuming it, and where it ends.
    fn peek(&self) -> Result<Option<(Element<'a>, usize)>, Error<'a>> {
        if self.rest().is_empty() {
            return Ok(None);
        }
        let (element, used) = Element::decode(self.rest(), self.at)?;
        Ok(Some((element, self.at + used)))
    }

    /// Context field `n`, its contents read as application type `tag`, or
    /// `None` when the next element is not that field.
    pub(crate) fn optional(&mut self, n: u8, tag: u8) -> Result<Option<Primitive<'a>>, Error<'a>> {
        match self.peek()? {
            Some((e, end))
                if e.header.class == Class::Context
                    && e.header.number == n
                    && !e.is_opening()
                    && !e.is_closing() =>
            {
                self.at = end;
                Primitive::decode_as(tag, e.content).map(Some)
            }
            _ => Ok(None),
        }
    }

    /// Context field `n`, which the grammar requires.
    pub(crate) fn required(&mut self, n: u8, tag: u8) -> Result<Primitive<'a>, Error<'a>> {
        self.optional(n, tag)?
            .ok_or_else(|| self.fail(ErrorKind::MissingField(n)))
    }

    /// The next element, which must be application-tagged.
    pub(crate) fn application(&mut self) -> Result<Primitive<'a>, Error<'a>> {
        match self.peek()? {
            Some((e, end)) if e.header.class == Class::Application => {
                self.at = end;
                Primitive::decode(&e)
            }
            _ => Err(self.fail(ErrorKind::MissingField(APPLICATION))),
        }
    }

    /// The octets between opening tag `n` and the closing tag that pairs
    /// with it, borrowed; nested constructed values stay inside.
    pub(crate) fn constructed(&mut self, n: u8) -> Result<&'a [u8], Error<'a>> {
        let start = match self.peek()? {
            Some((e, end)) if e.is_opening() && e.header.number == n => end,
            _ => return Err(self.fail(ErrorKind::MissingField(n))),
        };
        let mut at = start;
        let mut depth = 0usize;
        loop {
            let rest = self.buf.get(at..).unwrap_or(&[]);
            if rest.is_empty() {
                return Err(Error::new(ErrorKind::Unbalanced, self.rest(), self.at));
            }
            let (e, used) = Element::decode(rest, at)?;
            if e.is_closing() && depth == 0 {
                if e.header.number != n {
                    return Err(Error::new(ErrorKind::Unbalanced, rest, at));
                }
                self.at = at + used;
                return Ok(self.buf.get(start..at).unwrap_or(&[]));
            }
            if e.is_opening() {
                depth += 1;
            } else if e.is_closing() {
                depth -= 1;
            }
            at += used;
        }
    }

    /// Whether anything is left.
    pub(crate) fn is_empty(&self) -> bool {
        self.rest().is_empty()
    }

    /// Fails when octets remain after the grammar's last field.
    pub(crate) fn end(&self) -> Result<(), Error<'a>> {
        if self.is_empty() {
            Ok(())
        } else {
            Err(self.fail(ErrorKind::Trailing))
        }
    }
}

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
