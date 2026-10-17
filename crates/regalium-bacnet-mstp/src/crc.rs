// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The three checks MS/TP frames carry (Clause 9.2, Annex G, Annex T): an
//! 8-bit header CRC, a 16-bit data CRC, and for extended frames a 32-bit
//! CRC-32K — and the COBS encoding extended frames use.
//!
//! The algorithms are the standard's, as Wireshark's MS/TP dissector carries
//! them. Each check is sent complemented, so a receiver running it over the
//! field and its check lands on a fixed residue: [`HEADER_RESIDUE`],
//! [`DATA_RESIDUE`], [`CRC32K_RESIDUE`].

/// What the header CRC of a good header, check included, comes to.
pub const HEADER_RESIDUE: u8 = 0x55;
/// What the data CRC of good data, check included, comes to.
pub const DATA_RESIDUE: u16 = 0xF0B8;
/// What the CRC-32K of good extended data, check included, comes to.
pub const CRC32K_RESIDUE: u32 = 0x0843_323B;

/// The octet every COBS-encoded octet is exclusive-ored with, so an
/// encoded frame never contains the preamble's X'55' run as X'00'.
pub const COBS_MASK: u8 = 0x55;

/// Accumulates `data` into the header CRC (Annex G.1).
#[must_use]
pub const fn header(data: u8, crc: u8) -> u8 {
    let c = (crc ^ data) as u16;
    let c = c ^ (c << 1) ^ (c << 2) ^ (c << 3) ^ (c << 4) ^ (c << 5) ^ (c << 6) ^ (c << 7);
    let [low, _] = ((c & 0xfe) ^ ((c >> 8) & 1)).to_le_bytes();
    low
}

/// The header CRC of `octets`, complemented, as sent.
#[must_use]
pub fn header_check(octets: &[u8]) -> u8 {
    !octets.iter().fold(0xFF, |crc, b| header(*b, crc))
}

/// Accumulates `data` into the data CRC (Annex G.2): CRC-CCITT, reflected.
#[must_use]
pub const fn data(data: u8, crc: u16) -> u16 {
    let low = (crc & 0xff) ^ data as u16;
    (crc >> 8)
        ^ (low << 8)
        ^ (low << 3)
        ^ (low << 12)
        ^ (low >> 4)
        ^ (low & 0x0f)
        ^ ((low & 0x0f) << 7)
}

/// The data CRC of `octets`, complemented, as sent — low octet first.
#[must_use]
pub fn data_check(octets: &[u8]) -> [u8; 2] {
    (!octets.iter().fold(0xFFFF, |crc, b| data(*b, crc))).to_le_bytes()
}

/// Accumulates `data` into the CRC-32K (Annex T).
#[must_use]
pub const fn crc32k(data: u8, crc: u32) -> u32 {
    let (mut d, mut c, mut bit) = (data, crc, 0);
    while bit < 8 {
        c = if (d as u32 & 1) ^ (c & 1) == 1 {
            (c >> 1) ^ 0xEB31_D82E
        } else {
            c >> 1
        };
        d >>= 1;
        bit += 1;
    }
    c
}

/// COBS-encodes `data` (Annex T): no octet of the result is X'00' before
/// masking, and every octet is then exclusive-ored with [`COBS_MASK`].
#[must_use]
pub fn cobs_encode(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0];
    let mut code_at = 0;
    let mut code = 1u8;
    for &b in data {
        if b == 0 {
            if let Some(c) = out.get_mut(code_at) {
                *c = code;
            }
            code_at = out.len();
            out.push(0);
            code = 1;
            continue;
        }
        out.push(b);
        code += 1;
        if code == 0xFF {
            if let Some(c) = out.get_mut(code_at) {
                *c = code;
            }
            code_at = out.len();
            out.push(0);
            code = 1;
        }
    }
    if let Some(c) = out.get_mut(code_at) {
        *c = code;
    }
    out.iter().map(|b| b ^ COBS_MASK).collect()
}

/// Decodes what [`cobs_encode`] wrote; `None` for a code octet that runs
/// past the end or is zero.
#[must_use]
pub fn cobs_decode(encoded: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(encoded.len());
    let mut i = 0;
    while i < encoded.len() {
        let code = encoded.get(i)? ^ COBS_MASK;
        if code == 0 || i + usize::from(code) > encoded.len() {
            return None;
        }
        out.extend(
            encoded
                .get(i + 1..i + usize::from(code))?
                .iter()
                .map(|b| b ^ COBS_MASK),
        );
        i += usize::from(code);
        if code != 0xFF && i < encoded.len() {
            out.push(0);
        }
    }
    Some(out)
}
