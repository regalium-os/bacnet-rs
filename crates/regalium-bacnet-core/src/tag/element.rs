// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! An element: a header and the contents it announces. Borrowed from the
//! input, never copied, so decoding a datagram allocates nothing until a
//! caller asks for a `Vec` of them.

use super::header::{Header, Lvt};
use crate::error::{Error, ErrorKind};

/// One tagged element (§20.2.1). Borrows the buffer it was decoded from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Element<'a> {
    /// The header, as spelled.
    pub header: Header,
    /// The contents: `header.content_len()` octets, empty for a BOOLEAN or
    /// an opening or closing tag.
    pub content: &'a [u8],
}

impl<'a> Element<'a> {
    /// Decodes the element at the front of `buf`, returning it and the
    /// octets it occupied. Borrows `buf`; `offset` locates it for errors.
    ///
    /// # Errors
    ///
    /// As [`Header::decode`], and [`ErrorKind::Truncated`] when the contents
    /// run past the buffer.
    pub fn decode(buf: &'a [u8], offset: usize) -> Result<(Self, usize), Error<'a>> {
        let (header, at) = Header::decode(buf, offset)?;
        let len = usize::try_from(header.content_len()).unwrap_or(usize::MAX);
        let end = at.saturating_add(len);
        let content = buf.get(at..end).ok_or(Error::new(
            ErrorKind::Truncated {
                needed: end,
                available: buf.len(),
            },
            buf,
            offset,
        ))?;
        Ok((Self { header, content }, end))
    }

    /// Appends the element exactly as it was decoded.
    pub fn encode(&self, out: &mut Vec<u8>) {
        self.header.encode(out);
        out.extend_from_slice(self.content);
    }

    /// Whether this opens a constructed value.
    #[must_use]
    pub fn is_opening(&self) -> bool {
        self.header.lvt == Lvt::Opening
    }

    /// Whether this closes one.
    #[must_use]
    pub fn is_closing(&self) -> bool {
        self.header.lvt == Lvt::Closing
    }
}

/// The elements of a buffer, in order. Stops after the first error, which
/// it yields, so a malformed tail is reported once rather than misread.
#[derive(Clone, Debug)]
pub struct Elements<'a> {
    buf: &'a [u8],
    at: usize,
    failed: bool,
}

/// Iterates the elements of `buf`, borrowing it.
#[must_use]
pub fn elements(buf: &[u8]) -> Elements<'_> {
    Elements {
        buf,
        at: 0,
        failed: false,
    }
}

impl<'a> Iterator for Elements<'a> {
    type Item = Result<Element<'a>, Error<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        let rest = self.buf.get(self.at..).filter(|r| !r.is_empty())?;
        match Element::decode(rest, self.at) {
            Ok((element, used)) => {
                self.at += used;
                Some(Ok(element))
            }
            Err(e) => {
                self.failed = true;
                Some(Err(e))
            }
        }
    }
}

/// Every element of `buf`, with opening and closing tags checked to pair.
/// Borrows `buf`; allocates the `Vec` only.
///
/// # Errors
///
/// The first element that does not decode, or [`ErrorKind::Unbalanced`] at
/// the tag that breaks the pairing.
pub fn decode_all(buf: &[u8]) -> Result<Vec<Element<'_>>, Error<'_>> {
    let mut out = Vec::new();
    let mut open: Vec<u8> = Vec::new();
    let mut at = 0;
    while let Some(here) = buf.get(at..).filter(|r| !r.is_empty()) {
        let (element, used) = Element::decode(here, at)?;
        if element.is_opening() {
            open.push(element.header.number);
        } else if element.is_closing() && open.pop() != Some(element.header.number) {
            return Err(Error::new(ErrorKind::Unbalanced, here, at));
        }
        at += used;
        out.push(element);
    }
    if open.is_empty() {
        Ok(out)
    } else {
        Err(Error::new(
            ErrorKind::Unbalanced,
            buf.get(at..).unwrap_or_default(),
            at,
        ))
    }
}

/// The octets `elements` were decoded from: each exactly as spelled.
#[must_use]
pub fn encode_all(elements: &[Element<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    for e in elements {
        e.encode(&mut out);
    }
    out
}
