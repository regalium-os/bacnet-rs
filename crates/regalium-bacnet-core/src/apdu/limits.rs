// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What a confirmed request says the requester can take back (§20.1.2.4,
//! §20.1.2.5): the second octet of a Confirmed-Request-PDU.

/// The octet holding max-segments-accepted (bits 6–4) and
/// max-APDU-length-accepted (bits 3–0), kept as sent, bit 7 included.
///
/// A requester advertises these so the responder can size its answer:
/// answer too large for the requester's datalink and it is lost, segment
/// when the requester cannot reassemble and it is lost. The encoding is a
/// code rather than a length because one octet must carry both.
///
/// `Default` is 0: unspecified segments, 50-octet APDUs — the smallest any
/// device accepts, so a reply sized to it reaches anything.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Limits(pub u8);

/// How many segments a requester will reassemble (§20.1.2.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Segments {
    /// B'000': not said. A responder must not assume it may segment.
    Unspecified,
    /// At most this many: 2, 4, 8, 16, 32 or 64.
    AtMost(u8),
    /// B'111': more than 64.
    MoreThan64,
}

impl Limits {
    /// The octet for `segments` and the max-APDU `code` (0–15). Bit 7,
    /// which §20.1.2.4 reserves, is left 0.
    #[must_use]
    pub const fn new(segments: Segments, apdu_code: u8) -> Self {
        let s = match segments {
            Segments::Unspecified => 0,
            Segments::AtMost(n) => match n {
                0..=2 => 1,
                3..=4 => 2,
                5..=8 => 3,
                9..=16 => 4,
                17..=32 => 5,
                _ => 6,
            },
            Segments::MoreThan64 => 7,
        };
        Self((s << 4) | (apdu_code & 0x0F))
    }

    /// Max-segments-accepted.
    #[must_use]
    pub const fn max_segments(self) -> Segments {
        match (self.0 >> 4) & 0x07 {
            0 => Segments::Unspecified,
            7 => Segments::MoreThan64,
            n => Segments::AtMost(1 << n),
        }
    }

    /// The max-APDU-length-accepted code, 0–15.
    #[must_use]
    pub const fn apdu_code(self) -> u8 {
        self.0 & 0x0F
    }

    /// Max-APDU-length-accepted in octets, or `None` for codes 6–15,
    /// which §20.1.2.5 reserves. The lengths are those of the datalinks:
    /// 50 for MS/TP's minimum, 480 for ARCNET, 1476 for BACnet/IP's UDP.
    #[must_use]
    pub const fn max_apdu(self) -> Option<u16> {
        match self.apdu_code() {
            0 => Some(50),
            1 => Some(128),
            2 => Some(206),
            3 => Some(480),
            4 => Some(1024),
            5 => Some(1476),
            _ => None,
        }
    }

    /// Whether bit 7, reserved, was set.
    #[must_use]
    pub const fn reserved_bit(self) -> bool {
        self.0 & 0x80 != 0
    }
}
