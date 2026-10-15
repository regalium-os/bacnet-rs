// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Tag encoding (§20.2.1): the minimal-length form every device accepts.

use super::{ObjectId, Value};

/// Pushes a tag header for a primitive of `len` octets.
pub fn push_header(out: &mut Vec<u8>, number: u8, context: bool, len: usize) {
    let class = if context { 0x08 } else { 0 };
    let lvt = u8::try_from(len).ok().filter(|l| *l < 5).unwrap_or(5);
    push_first(out, number, class | lvt);
    if len >= 5 {
        if let Ok(byte) = u8::try_from(len)
            .map_err(drop)
            .and_then(|b| if b < 254 { Ok(b) } else { Err(()) })
        {
            out.push(byte);
        } else if let Ok(short) = u16::try_from(len) {
            out.push(254);
            out.extend(short.to_be_bytes());
        } else {
            out.push(255);
            out.extend(u32::try_from(len).unwrap_or(u32::MAX).to_be_bytes());
        }
    }
}

/// Pushes a context opening tag `[number]` (§20.2.1.3.2).
pub fn push_opening(out: &mut Vec<u8>, number: u8) {
    push_first(out, number, 0x0E);
}

/// Pushes a context closing tag `[number]`.
pub fn push_closing(out: &mut Vec<u8>, number: u8) {
    push_first(out, number, 0x0F);
}

fn push_first(out: &mut Vec<u8>, number: u8, low: u8) {
    if number < 15 {
        out.push((number << 4) | low);
    } else {
        out.extend([0xF0 | low, number]);
    }
}

/// Pushes an unsigned in the fewest octets (tag 2 when application-tagged).
pub fn push_unsigned(out: &mut Vec<u8>, number: u8, context: bool, value: u64) {
    let bytes = minimal(value);
    push_header(out, number, context, bytes.len());
    out.extend(bytes);
}

/// Pushes an object identifier (tag 12 when application-tagged).
pub fn push_object(out: &mut Vec<u8>, number: u8, context: bool, id: ObjectId) {
    push_header(out, number, context, 4);
    out.extend(id.raw().to_be_bytes());
}

/// Pushes one application-tagged value. `Value::Context` encodes nothing.
pub fn push_value(out: &mut Vec<u8>, value: &Value) {
    let raw = |out: &mut Vec<u8>, tag: u8, data: &[u8]| {
        push_header(out, tag, false, data.len());
        out.extend_from_slice(data);
    };
    match value {
        Value::Null => push_header(out, 0, false, 0),
        Value::Bool(b) => push_header(out, 1, false, usize::from(*b)),
        Value::Unsigned(v) => push_unsigned(out, 2, false, *v),
        Value::Signed(v) => raw(out, 3, &minimal_signed(*v)),
        Value::Real(v) => raw(out, 4, &v.to_be_bytes()),
        Value::Double(v) => raw(out, 5, &v.to_be_bytes()),
        Value::Octets(v) => raw(out, 6, v),
        Value::Text(s) => raw(out, 7, &[&[0u8][..], s.as_bytes()].concat()),
        Value::Bits(bits) => {
            // The leading octet counts unused bits in the last octet, 0..=7.
            let unused = u8::try_from((8 - bits.len() % 8) % 8).unwrap_or(0);
            let mut data = vec![unused];
            for chunk in bits.chunks(8) {
                data.push(
                    chunk
                        .iter()
                        .enumerate()
                        .fold(0, |b, (i, &on)| b | (u8::from(on) << (7 - i))),
                );
            }
            raw(out, 8, &data);
        }
        Value::Enumerated(v) => push_unsigned(out, 9, false, (*v).into()),
        Value::Date(d) => raw(out, 10, d),
        Value::Time(t) => raw(out, 11, t),
        Value::Object(id) => push_object(out, 12, false, *id),
        Value::Context(_) => {}
    }
}

fn minimal(value: u64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let skip = bytes.iter().take(7).take_while(|b| **b == 0).count();
    bytes.iter().skip(skip).copied().collect()
}

fn minimal_signed(value: i64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    // Drop a leading 0x00/0xFF while the next octet still carries the sign.
    let skip = bytes
        .windows(2)
        .take_while(|w| {
            matches!(w, [0x00, n] if n & 0x80 == 0) || matches!(w, [0xFF, n] if n & 0x80 != 0)
        })
        .count();
    bytes.iter().skip(skip).copied().collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::super::{decode, header};
    use super::*;

    fn round_trip(value: Value) {
        let mut out = Vec::new();
        push_value(&mut out, &value);
        assert_eq!(decode(&out), Some((value, out.len())));
    }

    #[test]
    fn every_value_kind_round_trips() {
        round_trip(Value::Null);
        round_trip(Value::Bool(true));
        round_trip(Value::Unsigned(1476));
        round_trip(Value::Signed(-129));
        round_trip(Value::Signed(127));
        round_trip(Value::Real(21.5));
        round_trip(Value::Text("Car 1 load".into()));
        round_trip(Value::Bits(vec![true, false, true]));
        round_trip(Value::Enumerated(62));
        round_trip(Value::Object(ObjectId::new(8, 599)));
    }

    #[test]
    fn a_string_longer_than_253_octets_uses_the_two_octet_length_escape() {
        let mut out = Vec::new();
        push_value(&mut out, &Value::Text("x".repeat(300)));
        assert_eq!(out.get(..4), Some(&[0x75, 254, 0x01, 0x2D][..]));
        let (h, size) = header(&out).unwrap();
        assert_eq!((h.len, size), (301, 4));
    }

    #[test]
    fn a_skipped_constructed_value_consumes_through_its_closing_tag() {
        let mut out = Vec::new();
        push_opening(&mut out, 3);
        push_opening(&mut out, 0);
        push_unsigned(&mut out, 1, true, 7);
        push_closing(&mut out, 0);
        push_closing(&mut out, 3);
        assert_eq!(decode(&out), Some((Value::Context(3), out.len())));
    }
}
