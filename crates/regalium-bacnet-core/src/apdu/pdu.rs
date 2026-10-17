// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The eight APDU types (§20.1.2–§20.1.9), as data.

use super::limits::Limits;

/// Octet 0, bit 3 of a Confirmed-Request or Complex-ACK: segmented message.
pub(crate) const SEG: u8 = 0x08;
/// Bit 2 of the same: more follows.
pub(crate) const MOR: u8 = 0x04;
/// Bit 1 of a Confirmed-Request: segmented response accepted.
pub(crate) const SA: u8 = 0x02;
/// Bit 1 of a Segment-ACK: negative acknowledgement.
pub(crate) const NAK: u8 = 0x02;
/// Bit 0 of a Segment-ACK or Abort: sent by the server.
pub(crate) const SRV: u8 = 0x01;

/// One APDU, decoded without copying: its bodies borrow the buffer
/// [`Apdu::decode`] was given.
///
/// # Lossless
///
/// [`Apdu::flags`] keeps the low four bits of octet 0 as sent, reserved
/// ones included, and [`Apdu::trailing`] keeps whatever followed a
/// fixed-length PDU. Only the segmented-message bit is written from the
/// [`Segment`] instead, so what is written can be read back; for a decoded
/// APDU they agree, and `encode(decode(x)) == x`.
///
/// `Copy`, holding only borrows: safe to share and send across threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Apdu<'a> {
    /// Bits 3–0 of octet 0. Their meaning depends on the type; the
    /// accessors on [`Apdu`] read them.
    pub flags: u8,
    /// The type and its fields.
    pub pdu: Pdu<'a>,
    /// Octets after a fixed-length PDU — Simple-ACK, Segment-ACK, Reject or
    /// Abort — which a conforming sender never adds. Empty for the types
    /// with a body, whose body runs to the end.
    pub trailing: &'a [u8],
}

/// The fields of each APDU type. A service body is left undecoded: only
/// the service grammar knows what its context tags mean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pdu<'a> {
    /// Type 0 (§20.1.2): a request that will be answered.
    ConfirmedRequest {
        /// What the requester can take back.
        limits: Limits,
        /// Pairs the request with its answer.
        invoke_id: u8,
        /// Present when the request is one segment of several.
        segment: Option<Segment>,
        /// The confirmed service choice (§21).
        service: u8,
        /// The service request.
        body: &'a [u8],
    },
    /// Type 1 (§20.1.3): a request nobody acknowledges — Who-Is, I-Am,
    /// unconfirmed COV notifications.
    UnconfirmedRequest {
        /// The unconfirmed service choice (§21).
        service: u8,
        /// The service request.
        body: &'a [u8],
    },
    /// Type 2 (§20.1.4): success, with nothing to return.
    SimpleAck {
        /// The request's invoke id.
        invoke_id: u8,
        /// The request's service choice.
        service: u8,
    },
    /// Type 3 (§20.1.5): success, with a result.
    ComplexAck {
        /// The request's invoke id.
        invoke_id: u8,
        /// Present when the result is one segment of several.
        segment: Option<Segment>,
        /// The request's service choice.
        service: u8,
        /// The service result.
        body: &'a [u8],
    },
    /// Type 4 (§20.1.6): acknowledges segments received so far.
    SegmentAck {
        /// The segmented message's invoke id.
        invoke_id: u8,
        /// The last segment received in order.
        sequence: u8,
        /// How many segments the receiver will take before acknowledging.
        window: u8,
    },
    /// Type 5 (§20.1.7): the service failed.
    Error {
        /// The request's invoke id.
        invoke_id: u8,
        /// The request's service choice.
        service: u8,
        /// The error: for most services an error class and code (§18).
        body: &'a [u8],
    },
    /// Type 6 (§20.1.8): the request was malformed or not understood.
    Reject {
        /// The request's invoke id.
        invoke_id: u8,
        /// Why (§20.1.8.3, `BACnetRejectReason`).
        reason: u8,
    },
    /// Type 7 (§20.1.9): the transaction was ended without completing.
    Abort {
        /// The transaction's invoke id.
        invoke_id: u8,
        /// Why (§20.1.9.3, `BACnetAbortReason`).
        reason: u8,
    },
}

/// The segmentation fields of a segmented Confirmed-Request or Complex-ACK
/// (§20.1.2.7, §20.1.2.8).
///
/// Segmentation exists because one answer — a `ReadPropertyMultiple` of a
/// whole device — can outgrow any one datalink frame; a window lets the
/// receiver pace the sender without acknowledging every segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Segment {
    /// This segment's number, from 0, modulo 256.
    pub sequence: u8,
    /// How many segments the sender proposes to send before waiting for a
    /// Segment-ACK, 1–127.
    pub window: u8,
}
