// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Wireshark-style breakdown of one BACnet/IP datagram: every field's octets
//! beside what they mean. Never fails: whatever cannot be dissected is shown
//! as raw trailing octets, because a malformed frame is exactly the one you
//! want to look at.

mod apdu;
mod envelope;
mod params;
mod schema;

/// One field: its octets, its nesting depth, and what it means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    /// The octets this field occupies, copied out of the datagram.
    pub bytes: Vec<u8>,
    /// How many constructed tags enclose it.
    pub depth: usize,
    /// What it means, e.g. `invoke id 1: pairs this reply with its request`.
    pub text: String,
}

/// Dissects a whole datagram, BVLL header to the last tag. Borrows the
/// datagram; every field copies its octets out.
#[must_use]
pub fn frame(datagram: &[u8]) -> Vec<Field> {
    let mut c = Cursor {
        buf: datagram,
        pos: 0,
        depth: 0,
        out: Vec::new(),
    };
    let _ = envelope::bvll(&mut c);
    if c.pos < c.buf.len() {
        let rest = c.buf.get(c.pos..).unwrap_or_default().to_vec();
        c.out.push(Field {
            text: format!("{} octets not dissected", rest.len()),
            bytes: rest,
            depth: 0,
        });
    }
    c.out
}

/// Octets as lower-case hex with no separator: `810a0011`.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        })
}

/// One line for a packet list: `Complex-ACK readProperty analog-input:1
/// present-value (85)`. Built from the dissection, so it agrees with it.
#[must_use]
pub fn summary(datagram: &[u8]) -> String {
    let fields = frame(datagram);
    let after = |prefix: &str, f: &Field| {
        f.text
            .strip_prefix(prefix)
            .map(|t| t.split_once(": ").map_or(t, |(_, rest)| rest).to_owned())
    };
    let mut parts: Vec<String> = Vec::new();
    for f in &fields {
        if let Some(t) = after("APDU type", f).or_else(|| after("service", f)) {
            parts.push(t);
        } else if let Some(t) = after("network message", f) {
            parts.push(format!("network {t}"));
        } else if let Some((_, v)) = f.text.split_once("object-identifier = ") {
            parts.push(v.to_owned());
        } else if let Some((_, v)) = f.text.split_once("property-identifier = ") {
            parts.push(v.to_owned());
        } else if let Some((_, v)) = f.text.split_once("i-am-device-identifier = ") {
            parts.push(v.to_owned());
        } else if f.text.contains("error-code = ") || f.text.contains("reason ") {
            parts.push(f.text.rsplit(" = ").next().unwrap_or(&f.text).to_owned());
        }
    }
    if parts.is_empty() {
        fields
            .get(1)
            .map_or_else(|| "not BACnet/IP".into(), |f| f.text.clone())
    } else {
        parts.join(" ")
    }
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
    depth: usize,
    out: Vec<Field>,
}

impl<'a> Cursor<'a> {
    fn peek(&self, n: usize) -> Option<&'a [u8]> {
        self.buf.get(self.pos..self.pos.checked_add(n)?)
    }
    fn take(&mut self, n: usize, text: impl Into<String>) -> Option<&'a [u8]> {
        let bytes = self.peek(n)?;
        self.pos += n;
        self.out.push(Field {
            bytes: bytes.to_vec(),
            depth: self.depth,
            text: text.into(),
        });
        Some(bytes)
    }
    fn byte(&mut self, text: impl FnOnce(u8) -> String) -> Option<u8> {
        let b = *self.peek(1)?.first()?;
        self.take(1, text(b))?;
        Some(b)
    }
    fn u16(&mut self, text: impl FnOnce(u16) -> String) -> Option<u16> {
        let v = u16::from_be_bytes(self.peek(2)?.try_into().ok()?);
        self.take(2, text(v))?;
        Some(v)
    }
}

#[cfg(test)]
mod tests;
