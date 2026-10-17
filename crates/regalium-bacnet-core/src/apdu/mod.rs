// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The application layer's PDU (§20.1): which of the eight message types a
//! datagram is, the transaction it belongs to, and the service it asks for.
//!
//! An [`Apdu`] is the envelope around a service request or result: the
//! type, the invoke id pairing a request with its answer, the segmentation
//! fields, and the service choice. The service body stays undecoded octets,
//! because only the service grammar knows what its context tags mean.
//!
//! # Why eight types
//!
//! `BACnet` separates success with and without a result (Simple-ACK,
//! Complex-ACK) from three ways of failing: Error when the service ran and
//! failed, Reject when the request could not be understood, Abort when the
//! transaction itself could not continue. A client retrying on the wrong one
//! retries a request that will never succeed, which is why they are types
//! rather than codes.
//!
//! # Lossless
//!
//! Decoding borrows the buffer and keeps octet 0's flags as sent, reserved
//! bits included, and any octets trailing a fixed-length PDU;
//! [`Apdu::encode`] reproduces the input exactly.

mod kind;
mod limits;
mod pdu;
mod traced;
mod wire;

pub use limits::{Limits, Segments};
pub use pdu::{Apdu, Pdu, Segment};
pub use traced::{Summary, decode_traced, encode_traced};
