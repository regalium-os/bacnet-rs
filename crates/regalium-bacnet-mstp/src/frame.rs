// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! An MS/TP frame (Clause 9.3): preamble X'55' X'FF', type, destination,
//! source, a two-octet length and the header CRC; then, when there is data,
//! the data and its CRC. Extended frames (types 32 and 33, Annex T) carry
//! COBS-encoded data and a COBS-encoded CRC-32K instead, which is what lets
//! a frame carry a whole 1497-octet NPDU.

use crate::crc::{
    CRC32K_RESIDUE, DATA_RESIDUE, HEADER_RESIDUE, cobs_decode, cobs_encode, crc32k, data,
    data_check, header, header_check,
};

/// Frame types (§9.3.1).
pub mod kind {
    /// Token.
    pub const TOKEN: u8 = 0x00;
    /// Poll For Master.
    pub const POLL_FOR_MASTER: u8 = 0x01;
    /// Reply To Poll For Master.
    pub const REPLY_TO_POLL_FOR_MASTER: u8 = 0x02;
    /// Test Request.
    pub const TEST_REQUEST: u8 = 0x03;
    /// Test Response.
    pub const TEST_RESPONSE: u8 = 0x04;
    /// `BACnet` Data Expecting Reply.
    pub const DATA_EXPECTING_REPLY: u8 = 0x05;
    /// `BACnet` Data Not Expecting Reply.
    pub const DATA_NOT_EXPECTING_REPLY: u8 = 0x06;
    /// Reply Postponed.
    pub const REPLY_POSTPONED: u8 = 0x07;
    /// `BACnet` Extended Data Expecting Reply (Annex T).
    pub const EXTENDED_EXPECTING_REPLY: u8 = 0x20;
    /// `BACnet` Extended Data Not Expecting Reply (Annex T).
    pub const EXTENDED_NOT_EXPECTING_REPLY: u8 = 0x21;
}

/// The destination address every station receives.
pub const BROADCAST: u8 = 0xFF;
/// The most data a frame other than an extended one carries (§9.3).
pub const MAX_DATA: usize = 501;

/// One frame, its data decoded. Owned: an extended frame's data exists only
/// once decoded.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Frame {
    /// The frame type: one of [`kind`], or a vendor's (128 and above).
    pub kind: u8,
    /// The destination station, or [`BROADCAST`].
    pub destination: u8,
    /// The source station.
    pub source: u8,
    /// The data, decoded; empty for a frame that carries none.
    pub data: Vec<u8>,
}

/// Why octets are not a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FrameError {
    /// Fewer octets than the header and its length say.
    Truncated {
        /// Octets needed.
        needed: usize,
    },
    /// Not X'55' X'FF'.
    NoPreamble,
    /// The header CRC does not check.
    HeaderCrc,
    /// The data CRC, or an extended frame's CRC-32K, does not check.
    DataCrc,
    /// An extended frame whose encoding does not decode.
    Encoding,
}

const fn extended(kind: u8) -> bool {
    matches!(
        kind,
        kind::EXTENDED_EXPECTING_REPLY | kind::EXTENDED_NOT_EXPECTING_REPLY
    )
}

impl Frame {
    /// A frame without data.
    #[must_use]
    pub const fn bare(kind: u8, destination: u8, source: u8) -> Self {
        Self {
            kind,
            destination,
            source,
            data: Vec::new(),
        }
    }

    /// The frame's octets, preamble to the last check.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let field = if extended(self.kind) {
            let mut enc = cobs_encode(&self.data);
            let crc = !enc.iter().fold(0xFFFF_FFFF, |c, b| crc32k(*b, c));
            enc.extend(cobs_encode(&crc.to_le_bytes()));
            enc
        } else {
            let mut d = self.data.clone();
            if !d.is_empty() {
                d.extend(data_check(&self.data));
            }
            d
        };
        // An extended frame's length field is two less than what follows,
        // so a station that does not know the type skips it as data.
        let length = if field.is_empty() { 0 } else { field.len() - 2 };
        let [hi, lo] = u16::try_from(length).unwrap_or(u16::MAX).to_be_bytes();
        let head = [self.kind, self.destination, self.source, hi, lo];
        let mut out = vec![0x55, 0xFF];
        out.extend(head);
        out.push(header_check(&head));
        out.extend(field);
        out
    }

    /// Decodes the frame at the front of `octets`, returning it and the
    /// octets it used.
    ///
    /// # Errors
    ///
    /// [`FrameError`]: no preamble, too short, a check that fails.
    pub fn decode(octets: &[u8]) -> Result<(Self, usize), FrameError> {
        let short = |needed| FrameError::Truncated { needed };
        let Some([p0, p1, kind, destination, source, hi, lo, crc]) =
            octets.get(..8).and_then(|h| <[u8; 8]>::try_from(h).ok())
        else {
            return Err(short(8));
        };
        if (p0, p1) != (0x55, 0xFF) {
            return Err(FrameError::NoPreamble);
        }
        if [kind, destination, source, hi, lo, crc]
            .iter()
            .fold(0xFF, |c, b| header(*b, c))
            != HEADER_RESIDUE
        {
            return Err(FrameError::HeaderCrc);
        }
        let length = usize::from(u16::from_be_bytes([hi, lo]));
        if length == 0 {
            return Ok((Self::bare(kind, destination, source), 8));
        }
        let end = 8 + length + 2;
        let field = octets.get(8..end).ok_or(short(end))?;
        let data = if extended(kind) {
            let split = field.len().saturating_sub(5);
            let (enc, enc_crc) = (
                field.get(..split).unwrap_or(&[]),
                field.get(split..).unwrap_or(&[]),
            );
            let crc = cobs_decode(enc_crc).ok_or(FrameError::Encoding)?;
            let check = enc
                .iter()
                .chain(&crc)
                .fold(0xFFFF_FFFF, |c, b| crc32k(*b, c));
            if crc.len() != 4 || check != CRC32K_RESIDUE {
                return Err(FrameError::DataCrc);
            }
            cobs_decode(enc).ok_or(FrameError::Encoding)?
        } else {
            if field.iter().fold(0xFFFF, |c, b| data(*b, c)) != DATA_RESIDUE {
                return Err(FrameError::DataCrc);
            }
            field.get(..length).unwrap_or(&[]).to_vec()
        };
        Ok((
            Self {
                kind,
                destination,
                source,
                data,
            },
            end,
        ))
    }
}
