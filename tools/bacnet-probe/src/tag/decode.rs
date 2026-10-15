// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Tag decoding (§20.2.1). Every function borrows its input and returns how
//! many octets it consumed; values are copied out, so nothing returned
//! borrows the datagram.

use super::{Header, ObjectId, Value};

/// Decodes the tag header at the front of `buf`; returns it and its size.
#[must_use]
pub fn header(buf: &[u8]) -> Option<(Header, usize)> {
    let first = *buf.first()?;
    let context = first & 0x08 != 0;
    let lvt = first & 0x07;
    let (number, mut pos) = if first >> 4 == 0x0F {
        (*buf.get(1)?, 2)
    } else {
        (first >> 4, 1)
    };
    let opening = context && lvt == 6;
    let closing = context && lvt == 7;
    // An application boolean carries its value in the length field (§20.2.3).
    let len = if opening || closing || (!context && number == 1) {
        0
    } else if lvt == 5 {
        // Extended length (§20.2.1.3.1): 254 → u16 follows, 255 → u32 follows.
        let n = *buf.get(pos)?;
        pos += 1;
        let width = match n {
            254 => 2,
            255 => 4,
            n => {
                return Some((
                    Header {
                        number,
                        context,
                        lvt,
                        len: n.into(),
                        opening,
                        closing,
                    },
                    pos,
                ));
            }
        };
        let len = usize::try_from(be(buf.get(pos..pos + width)?)).ok()?;
        pos += width;
        len
    } else {
        lvt.into()
    };
    Some((
        Header {
            number,
            context,
            lvt,
            len,
            opening,
            closing,
        },
        pos,
    ))
}

/// Decodes one tagged item at the front of `buf`; returns it and its size.
/// A context opening tag is skipped through its matching closing tag.
#[must_use]
pub fn decode(buf: &[u8]) -> Option<(Value, usize)> {
    let (h, pos) = header(buf)?;
    if h.closing {
        return None;
    }
    if h.opening {
        return Some((Value::Context(h.number), skip_constructed(buf)?));
    }
    let end = pos.checked_add(h.len)?;
    let data = buf.get(pos..end)?;
    if h.context {
        return Some((Value::Context(h.number), end));
    }
    let value = match h.number {
        0 => Value::Null,
        1 => Value::Bool(h.lvt != 0),
        2 if data.len() <= 8 => Value::Unsigned(be(data)),
        3 if data.len() <= 8 => Value::Signed(signed(data)),
        4 => Value::Real(f32::from_be_bytes(data.try_into().ok()?)),
        5 => Value::Double(f64::from_be_bytes(data.try_into().ok()?)),
        7 => Value::Text(text(data)),
        8 => Value::Bits(bits(data)),
        9 if data.len() <= 4 => Value::Enumerated(u32::try_from(be(data)).ok()?),
        10 => Value::Date(data.try_into().ok()?),
        11 => Value::Time(data.try_into().ok()?),
        12 if data.len() == 4 => Value::Object(ObjectId::from_raw(u32::try_from(be(data)).ok()?)),
        _ => Value::Octets(data.to_vec()),
    };
    Some((value, end))
}

/// Decodes a context-tagged unsigned; returns tag number, value and size.
#[must_use]
pub fn decode_context_unsigned(buf: &[u8]) -> Option<(u8, u64, usize)> {
    let (h, pos) = header(buf)?;
    let end = pos.checked_add(h.len)?;
    (h.context && h.len <= 8).then_some(())?;
    Some((h.number, be(buf.get(pos..end)?), end))
}

fn skip_constructed(buf: &[u8]) -> Option<usize> {
    let (mut pos, mut depth) = (0usize, 0usize);
    loop {
        let (h, n) = header(buf.get(pos..)?)?;
        pos += n;
        if h.opening {
            depth += 1;
        } else if h.closing {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(pos);
            }
        } else {
            pos = pos.checked_add(h.len)?;
        }
    }
}

pub(crate) fn be(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0, |acc, b| (acc << 8) | u64::from(*b))
}

/// Two's complement of up to 8 octets: shift the value to the top of an
/// i64, then arithmetic-shift back down to extend the sign.
fn signed(data: &[u8]) -> i64 {
    let bits = data.len().saturating_mul(8).min(64);
    if bits == 0 {
        return 0;
    }
    let shift = u32::try_from(64 - bits).unwrap_or(0);
    (be(data) << shift).cast_signed() >> shift
}

pub(crate) fn text(data: &[u8]) -> String {
    let (charset, rest) = data.split_first().unwrap_or((&0, &[]));
    match charset {
        // UCS-2 big-endian.
        4 => String::from_utf16_lossy(
            &rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect::<Vec<_>>(),
        ),
        // ISO 8859-1 maps byte-for-byte onto the first 256 code points.
        5 => rest.iter().map(|&b| char::from(b)).collect(),
        // 0 is ANSI X3.4 / UTF-8; anything else, best effort.
        _ => String::from_utf8_lossy(rest).into_owned(),
    }
}

fn bits(data: &[u8]) -> Vec<bool> {
    let (unused, rest) = data.split_first().unwrap_or((&0, &[]));
    let total = (rest.len() * 8).saturating_sub(usize::from(*unused));
    (0..total)
        .map(|i| rest.get(i / 8).is_some_and(|b| b & (0x80 >> (i % 8)) != 0))
        .collect()
}
