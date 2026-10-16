// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Service codecs (Clauses 13–16): what goes inside an APDU, as plain data,
//! for reading, writing, creating and deleting.
//!
//! Pure functions, shared by the client and the simulator so the two cannot
//! disagree about the wire. Decoders borrow the APDU body and copy values
//! out; nothing returned borrows the datagram.

pub mod cov;
pub mod object;
pub mod read;
pub mod rpm;
pub mod who;
pub mod write;

mod reader;

pub(crate) use reader::Reader;

use crate::tag::{self, Value};

/// Confirmed service choices (§21, `BACnetConfirmedServiceChoice`).
pub mod confirmed {
    /// `ConfirmedCOVNotification`.
    pub const COV_NOTIFICATION: u8 = 1;
    /// `SubscribeCOV`.
    pub const SUBSCRIBE_COV: u8 = 5;
    /// `CreateObject`.
    pub const CREATE_OBJECT: u8 = 10;
    /// `DeleteObject`.
    pub const DELETE_OBJECT: u8 = 11;
    /// `ReadProperty`.
    pub const READ_PROPERTY: u8 = 12;
    /// `ReadPropertyMultiple`.
    pub const READ_PROPERTY_MULTIPLE: u8 = 14;
    /// `WriteProperty`.
    pub const WRITE_PROPERTY: u8 = 15;
    /// `WritePropertyMultiple`.
    pub const WRITE_PROPERTY_MULTIPLE: u8 = 16;
}

/// Unconfirmed service choices (§21, `BACnetUnconfirmedServiceChoice`).
pub mod unconfirmed {
    /// I-Am.
    pub const I_AM: u8 = 0;
    /// I-Have.
    pub const I_HAVE: u8 = 1;
    /// `UnconfirmedCOVNotification`.
    pub const COV_NOTIFICATION: u8 = 2;
    /// Who-Has.
    pub const WHO_HAS: u8 = 7;
    /// Who-Is.
    pub const WHO_IS: u8 = 8;
}

/// A property, and optionally one element of it (`BACnetPropertyReference`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropertyRef {
    /// `BACnetPropertyIdentifier`.
    pub property: u32,
    /// Array index; `Some(0)` is the array's length.
    pub index: Option<u32>,
}

impl PropertyRef {
    /// The whole property.
    #[must_use]
    pub const fn whole(property: u32) -> Self {
        Self {
            property,
            index: None,
        }
    }
}

/// A property's value, or the device's `(error-class, error-code)`.
pub type PropertyResult = Result<Vec<Value>, (u32, u32)>;

/// A Confirmed-Request header (§20.1.2): unsegmented, any number of
/// segments accepted, replies up to 1476 octets.
#[must_use]
pub fn confirmed_request(invoke: u8, service: u8, body: &[u8]) -> Vec<u8> {
    [&[0x00, 0x05, invoke, service][..], body].concat()
}

/// An Unconfirmed-Request (§20.1.3).
#[must_use]
pub fn unconfirmed_request(service: u8, body: &[u8]) -> Vec<u8> {
    [&[0x10, service][..], body].concat()
}

/// A Simple-ACK (§20.1.4).
#[must_use]
pub fn simple_ack(invoke: u8, service: u8) -> Vec<u8> {
    vec![0x20, invoke, service]
}

/// A Complex-ACK, unsegmented (§20.1.5).
#[must_use]
pub fn complex_ack(invoke: u8, service: u8, body: &[u8]) -> Vec<u8> {
    [&[0x30, invoke, service][..], body].concat()
}

/// An Error-PDU (§20.1.7) carrying error class and code.
#[must_use]
pub fn error(invoke: u8, service: u8, class: u32, code: u32) -> Vec<u8> {
    let mut out = vec![0x50, invoke, service];
    tag::push_value(&mut out, &Value::Enumerated(class));
    tag::push_value(&mut out, &Value::Enumerated(code));
    out
}

/// A Reject-PDU (§20.1.8), e.g. reason 9 unrecognized-service.
#[must_use]
pub fn reject(invoke: u8, reason: u8) -> Vec<u8> {
    vec![0x60, invoke, reason]
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod write_tests;
