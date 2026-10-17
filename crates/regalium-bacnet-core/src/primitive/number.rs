// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Integers on the wire: big-endian, as few octets as the value needs
//! (§20.2.4 Unsigned, §20.2.5 Signed, §20.2.11 Enumerated).

/// A big-endian unsigned of up to eight octets.
pub(crate) fn unsigned(octets: &[u8]) -> u64 {
    octets.iter().fold(0, |acc, b| (acc << 8) | u64::from(*b))
}

/// A big-endian two's complement of up to eight octets, sign-extended.
pub(crate) fn signed(octets: &[u8]) -> i64 {
    let bits = octets.len().saturating_mul(8).min(64);
    if bits == 0 {
        return 0;
    }
    let shift = u32::try_from(64 - bits).unwrap_or(0);
    (unsigned(octets) << shift).cast_signed() >> shift
}

/// The fewest octets that hold `v`, at least one.
pub(crate) fn unsigned_octets(v: u64) -> Vec<u8> {
    let bytes = v.to_be_bytes();
    let skip = bytes.iter().take_while(|b| **b == 0).count().min(7);
    bytes.get(skip..).unwrap_or_default().to_vec()
}

/// The fewest octets that hold `v` with its sign, at least one: a leading
/// octet is dropped only while the next one's top bit still says the same
/// sign.
pub(crate) fn signed_octets(v: i64) -> Vec<u8> {
    let bytes = v.to_be_bytes();
    let mut start = 0;
    while start < 7 {
        let (Some(&lead), Some(&next)) = (bytes.get(start), bytes.get(start + 1)) else {
            break;
        };
        let redundant = (lead == 0x00 && next & 0x80 == 0) || (lead == 0xFF && next & 0x80 != 0);
        if !redundant {
            break;
        }
        start += 1;
    }
    bytes.get(start..).unwrap_or_default().to_vec()
}
