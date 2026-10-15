// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A cursor over a service body that expects tags in order.
//!
//! Every method borrows the body and returns `None` rather than panicking
//! when the next tag is not what the grammar says, so a malformed reply is a
//! decode failure the caller reports, never an abort.

use crate::tag::{self, Header, ObjectId, Value};

pub(crate) struct Reader<'a> {
    buf: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    fn peek(&self) -> Option<(Header, usize)> {
        tag::header(self.buf)
    }

    fn advance(&mut self, n: usize) -> Option<()> {
        self.buf = self.buf.get(n..)?;
        Some(())
    }

    /// The data of context tag `n`, consuming it.
    fn context(&mut self, n: u8) -> Option<&'a [u8]> {
        let (h, size) = self.peek()?;
        if !h.context || h.opening || h.closing || h.number != n {
            return None;
        }
        let end = size.checked_add(h.len)?;
        let data = self.buf.get(size..end)?;
        self.advance(end)?;
        Some(data)
    }

    /// Whether the next tag is context tag `n` (primitive).
    pub(crate) fn at_context(&self, n: u8) -> bool {
        self.peek()
            .is_some_and(|(h, _)| h.context && !h.opening && !h.closing && h.number == n)
    }

    pub(crate) fn unsigned(&mut self, n: u8) -> Option<u64> {
        let data = self.context(n)?;
        (data.len() <= 8).then(|| tag::be(data))
    }

    /// The outer `None` is a malformed body; the inner one, a parameter the
    /// grammar allows to be absent.
    #[allow(clippy::option_option)]
    pub(crate) fn optional_unsigned(&mut self, n: u8) -> Option<Option<u64>> {
        if self.at_context(n) {
            self.unsigned(n).map(Some)
        } else {
            Some(None)
        }
    }

    pub(crate) fn object(&mut self, n: u8) -> Option<ObjectId> {
        let data: [u8; 4] = self.context(n)?.try_into().ok()?;
        Some(ObjectId::from_raw(u32::from_be_bytes(data)))
    }

    pub(crate) fn boolean(&mut self, n: u8) -> Option<bool> {
        Some(*self.context(n)?.first()? != 0)
    }

    pub(crate) fn text(&mut self, n: u8) -> Option<String> {
        self.context(n).map(tag::text)
    }

    pub(crate) fn at_opening(&self, n: u8) -> bool {
        self.peek().is_some_and(|(h, _)| h.opening && h.number == n)
    }

    pub(crate) fn at_closing(&self, n: u8) -> bool {
        self.peek().is_some_and(|(h, _)| h.closing && h.number == n)
    }

    pub(crate) fn opening(&mut self, n: u8) -> Option<()> {
        let (_, size) = self.peek()?;
        if !self.at_opening(n) {
            return None;
        }
        self.advance(size)
    }

    pub(crate) fn closing(&mut self, n: u8) -> Option<()> {
        let (_, size) = self.peek()?;
        if !self.at_closing(n) {
            return None;
        }
        self.advance(size)
    }

    /// One application-tagged value.
    pub(crate) fn app(&mut self) -> Option<Value> {
        let (h, _) = self.peek()?;
        if h.context {
            return None;
        }
        let (value, used) = tag::decode(self.buf)?;
        self.advance(used)?;
        Some(value)
    }

    /// Values up to and including closing tag `n`. A nested constructed
    /// value is skipped whole and kept as `Value::Context`.
    pub(crate) fn values_until(&mut self, n: u8) -> Option<Vec<Value>> {
        let mut values = Vec::new();
        while !self.at_closing(n) {
            let (value, used) = tag::decode(self.buf)?;
            values.push(value);
            self.advance(used)?;
        }
        self.closing(n)?;
        Some(values)
    }
}
