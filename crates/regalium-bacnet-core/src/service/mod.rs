// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Services (Clauses 13–17): what a request asks and what its answer says.
//!
//! Each service is a struct with `decode` and `encode` for its body — the
//! octets after the APDU's service choice. [`Service::decode`] reads an
//! [`Apdu`] as whichever of them it carries.
//!
//! # Built so far
//!
//! Who-Is and I-Am (§16.1, §16.10), `ReadProperty` and its ACK (§15.5), and
//! the plain error a service returns (§18). The rest of Clauses 13–17 join
//! as the roadmap reaches them; until then an APDU carrying one decodes to
//! [`Service::Undecoded`], and its octets are still in the APDU.
//!
//! # Lossless, one layer down
//!
//! A service is a reading of the tags beneath it and re-encodes
//! canonically: a device that wrote an instance in four octets where two
//! would do is answered in two. The octets as sent are never lost — the
//! [`Apdu`] that carried them keeps them, and the tag layer reproduces them
//! — and a value whose type only its property knows stays undecoded
//! octets, as [`ReadPropertyAck::value`] does.

mod fields;
mod i_am;
mod read_property;
mod service_error;
mod traced;
mod who_is;

pub use i_am::IAm;
pub use read_property::{ReadProperty, ReadPropertyAck};
pub use service_error::ServiceError;
pub use traced::decode_traced;
pub use who_is::WhoIs;

use crate::apdu::{Apdu, Pdu};
use crate::error::Error;

/// Unconfirmed service choices (§21, `BACnetUnconfirmedServiceChoice`).
pub mod unconfirmed {
    /// I-Am.
    pub const I_AM: u8 = 0;
    /// Who-Has.
    pub const WHO_HAS: u8 = 7;
    /// Who-Is.
    pub const WHO_IS: u8 = 8;
}

/// Confirmed service choices (§21, `BACnetConfirmedServiceChoice`).
pub mod confirmed {
    /// `ReadProperty`.
    pub const READ_PROPERTY: u8 = 12;
    /// `ReadPropertyMultiple`.
    pub const READ_PROPERTY_MULTIPLE: u8 = 14;
    /// `WriteProperty`.
    pub const WRITE_PROPERTY: u8 = 15;
}

/// The service an APDU carries, decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service<'a> {
    /// An unconfirmed Who-Is.
    WhoIs(WhoIs),
    /// An unconfirmed I-Am.
    IAm(IAm),
    /// A confirmed `ReadProperty` request.
    ReadProperty(ReadProperty),
    /// An unsegmented Complex-ACK to `ReadProperty`. A segmented one is
    /// reassembled by [`crate::tsm`] and decoded with
    /// [`ReadPropertyAck::decode`] directly.
    ReadPropertyAck(ReadPropertyAck<'a>),
    /// An Error PDU for `ReadProperty`.
    Error {
        /// The service that failed.
        service: u8,
        /// Why.
        error: ServiceError,
    },
    /// A PDU this build does not read as a service yet, or one that carries
    /// none: Simple-ACK, Segment-ACK, Reject, Abort, a segment.
    Undecoded,
}

impl<'a> Service<'a> {
    /// Reads `apdu` as the service it carries. Borrows what the APDU borrows.
    ///
    /// # Errors
    ///
    /// The service's own decode error, when its body breaks its grammar.
    pub fn decode(apdu: &Apdu<'a>) -> Result<Self, Error<'a>> {
        use confirmed::READ_PROPERTY;
        use unconfirmed::{I_AM, WHO_IS};
        Ok(match apdu.pdu {
            Pdu::UnconfirmedRequest {
                service: WHO_IS,
                body,
            } => Self::WhoIs(WhoIs::decode(body)?),
            Pdu::UnconfirmedRequest {
                service: I_AM,
                body,
            } => Self::IAm(IAm::decode(body)?),
            Pdu::ConfirmedRequest {
                service: READ_PROPERTY,
                segment: None,
                body,
                ..
            } => Self::ReadProperty(ReadProperty::decode(body)?),
            Pdu::ComplexAck {
                service: READ_PROPERTY,
                segment: None,
                body,
                ..
            } => Self::ReadPropertyAck(ReadPropertyAck::decode(body)?),
            Pdu::Error {
                service: READ_PROPERTY,
                body,
                ..
            } => Self::Error {
                service: READ_PROPERTY,
                error: ServiceError::decode(body)?,
            },
            _ => Self::Undecoded,
        })
    }

    /// The service's name, as the span and the corpus spell it.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::WhoIs(_) => "who-is",
            Self::IAm(_) => "i-am",
            Self::ReadProperty(_) => "read-property",
            Self::ReadPropertyAck(_) => "read-property-ack",
            Self::Error { .. } => "error",
            Self::Undecoded => "undecoded",
        }
    }
}
