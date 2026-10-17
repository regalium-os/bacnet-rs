// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What an APDU is: its type, and the flags of octet 0 read for that type
//! (§20.1.2.1–§20.1.9.1).

use super::pdu::{Apdu, MOR, NAK, Pdu, SA, SRV};

impl Apdu<'_> {
    /// The type number, 0–7, of octet 0's high nibble.
    #[must_use]
    pub const fn pdu_type(&self) -> u8 {
        match self.pdu {
            Pdu::ConfirmedRequest { .. } => 0,
            Pdu::UnconfirmedRequest { .. } => 1,
            Pdu::SimpleAck { .. } => 2,
            Pdu::ComplexAck { .. } => 3,
            Pdu::SegmentAck { .. } => 4,
            Pdu::Error { .. } => 5,
            Pdu::Reject { .. } => 6,
            Pdu::Abort { .. } => 7,
        }
    }

    /// The type's name as §20.1 writes it, lower-case and hyphenated.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self.pdu_type() {
            0 => "confirmed-request",
            1 => "unconfirmed-request",
            2 => "simple-ack",
            3 => "complex-ack",
            4 => "segment-ack",
            5 => "error",
            6 => "reject",
            _ => "abort",
        }
    }

    /// More segments follow this one (`MOR`). Only a segmented
    /// Confirmed-Request or Complex-ACK can say so.
    #[must_use]
    pub const fn more_follows(&self) -> bool {
        matches!(self.pdu_type(), 0 | 3) && self.flags & MOR != 0
    }

    /// The requester can reassemble a segmented answer (`SA`). Only a
    /// Confirmed-Request says so.
    #[must_use]
    pub const fn accepts_segmented_response(&self) -> bool {
        self.pdu_type() == 0 && self.flags & SA != 0
    }

    /// A Segment-ACK asking for segments again from its sequence on (`NAK`).
    #[must_use]
    pub const fn is_negative(&self) -> bool {
        self.pdu_type() == 4 && self.flags & NAK != 0
    }

    /// A Segment-ACK or Abort sent by the server rather than the client
    /// (`SRV`). The invoke id alone cannot tell them apart.
    #[must_use]
    pub const fn from_server(&self) -> bool {
        matches!(self.pdu_type(), 4 | 7) && self.flags & SRV != 0
    }
}
