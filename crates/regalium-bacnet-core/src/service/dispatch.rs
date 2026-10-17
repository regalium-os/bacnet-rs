// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Reading an APDU as the service it carries.

use super::confirmed as c;
use super::unconfirmed as u;
use super::{
    CovNotification, CreateObject, CreateObjectError, EventNotification, IAm, IHave, ObjectBody,
    ReadProperty, ReadPropertyAck, ReadPropertyMultiple, ReadPropertyMultipleAck, ServiceError,
    SubscribeCov, SubscribeCovProperty, WhoHas, WhoIs, WriteMultipleError, WriteProperty,
    WritePropertyMultiple,
};
use crate::apdu::{Apdu, Pdu};
use crate::error::Error;
use crate::primitive::ObjectIdentifier;

/// The service an APDU carries, decoded. Non-exhaustive: services join as
/// the roadmap reaches them, and a caller's `match` must not break.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Service<'a> {
    /// Who-Is.
    WhoIs(WhoIs),
    /// I-Am.
    IAm(IAm),
    /// Who-Has.
    WhoHas(WhoHas<'a>),
    /// I-Have.
    IHave(IHave<'a>),
    /// A `ReadProperty` request.
    ReadProperty(ReadProperty),
    /// An unsegmented `ReadProperty` Complex-ACK. A segmented one is
    /// reassembled by [`crate::tsm`] and decoded from its result directly.
    ReadPropertyAck(ReadPropertyAck<'a>),
    /// A `ReadPropertyMultiple` request.
    ReadPropertyMultiple(ReadPropertyMultiple),
    /// An unsegmented `ReadPropertyMultiple` Complex-ACK.
    ReadPropertyMultipleAck(ReadPropertyMultipleAck<'a>),
    /// A `WriteProperty` request.
    WriteProperty(WriteProperty<'a>),
    /// A `WritePropertyMultiple` request.
    WritePropertyMultiple(WritePropertyMultiple<'a>),
    /// A `SubscribeCOV` request.
    SubscribeCov(SubscribeCov),
    /// A `SubscribeCOVProperty` request.
    SubscribeCovProperty(SubscribeCovProperty),
    /// A COV notification; a confirmed one must be acknowledged (§13.7).
    CovNotification {
        /// Whether it came as a confirmed request.
        confirmed: bool,
        /// What changed.
        notification: CovNotification<'a>,
    },
    /// An event notification; a confirmed one must be acknowledged.
    EventNotification {
        /// Whether it came as a confirmed request.
        confirmed: bool,
        /// The event.
        notification: EventNotification<'a>,
    },
    /// A `CreateObject` request.
    CreateObject(CreateObject<'a>),
    /// A `CreateObject` Complex-ACK: the object made.
    CreateObjectAck(ObjectIdentifier),
    /// A `DeleteObject` request.
    DeleteObject(ObjectIdentifier),
    /// An Error PDU with the plain error (§18).
    Error {
        /// The service that failed.
        service: u8,
        /// Why.
        error: ServiceError,
    },
    /// The Error PDU of a `CreateObject`.
    CreateObjectError(CreateObjectError),
    /// The Error PDU of a `WritePropertyMultiple`.
    WriteMultipleError(WriteMultipleError),
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
        Ok(match apdu.pdu {
            Pdu::UnconfirmedRequest { service, body } => match service {
                u::WHO_IS => Self::WhoIs(WhoIs::decode(body)?),
                u::I_AM => Self::IAm(IAm::decode(body)?),
                u::WHO_HAS => Self::WhoHas(WhoHas::decode(body)?),
                u::I_HAVE => Self::IHave(IHave::decode(body)?),
                u::COV_NOTIFICATION => Self::CovNotification {
                    confirmed: false,
                    notification: CovNotification::decode(body)?,
                },
                u::EVENT_NOTIFICATION => Self::EventNotification {
                    confirmed: false,
                    notification: EventNotification::decode(body)?,
                },
                _ => Self::Undecoded,
            },
            Pdu::ConfirmedRequest {
                service,
                segment: None,
                body,
                ..
            } => match service {
                c::READ_PROPERTY => Self::ReadProperty(ReadProperty::decode(body)?),
                c::READ_PROPERTY_MULTIPLE => {
                    Self::ReadPropertyMultiple(ReadPropertyMultiple::decode(body)?)
                }
                c::WRITE_PROPERTY => Self::WriteProperty(WriteProperty::decode(body)?),
                c::WRITE_PROPERTY_MULTIPLE => {
                    Self::WritePropertyMultiple(WritePropertyMultiple::decode(body)?)
                }
                c::SUBSCRIBE_COV => Self::SubscribeCov(SubscribeCov::decode(body)?),
                c::SUBSCRIBE_COV_PROPERTY => {
                    Self::SubscribeCovProperty(SubscribeCovProperty::decode(body)?)
                }
                c::COV_NOTIFICATION => Self::CovNotification {
                    confirmed: true,
                    notification: CovNotification::decode(body)?,
                },
                c::EVENT_NOTIFICATION => Self::EventNotification {
                    confirmed: true,
                    notification: EventNotification::decode(body)?,
                },
                c::CREATE_OBJECT => Self::CreateObject(CreateObject::decode(body)?),
                c::DELETE_OBJECT => Self::DeleteObject(ObjectBody::decode(body)?.0),
                _ => Self::Undecoded,
            },
            Pdu::ComplexAck {
                service,
                segment: None,
                body,
                ..
            } => match service {
                c::READ_PROPERTY => Self::ReadPropertyAck(ReadPropertyAck::decode(body)?),
                c::READ_PROPERTY_MULTIPLE => {
                    Self::ReadPropertyMultipleAck(ReadPropertyMultipleAck::decode(body)?)
                }
                c::CREATE_OBJECT => Self::CreateObjectAck(ObjectBody::decode(body)?.0),
                _ => Self::Undecoded,
            },
            Pdu::Error { service, body, .. } => match service {
                c::CREATE_OBJECT => Self::CreateObjectError(CreateObjectError::decode(body)?),
                c::WRITE_PROPERTY_MULTIPLE => {
                    Self::WriteMultipleError(WriteMultipleError::decode(body)?)
                }
                c::READ_PROPERTY
                | c::READ_PROPERTY_MULTIPLE
                | c::WRITE_PROPERTY
                | c::SUBSCRIBE_COV
                | c::SUBSCRIBE_COV_PROPERTY
                | c::COV_NOTIFICATION
                | c::EVENT_NOTIFICATION
                | c::DELETE_OBJECT => Self::Error {
                    service,
                    error: ServiceError::decode(body)?,
                },
                _ => Self::Undecoded,
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
            Self::WhoHas(_) => "who-has",
            Self::IHave(_) => "i-have",
            Self::ReadProperty(_) => "read-property",
            Self::ReadPropertyAck(_) => "read-property-ack",
            Self::ReadPropertyMultiple(_) => "read-property-multiple",
            Self::ReadPropertyMultipleAck(_) => "read-property-multiple-ack",
            Self::WriteProperty(_) => "write-property",
            Self::WritePropertyMultiple(_) => "write-property-multiple",
            Self::SubscribeCov(_) => "subscribe-cov",
            Self::SubscribeCovProperty(_) => "subscribe-cov-property",
            Self::CovNotification { .. } => "cov-notification",
            Self::EventNotification { .. } => "event-notification",
            Self::CreateObject(_) => "create-object",
            Self::CreateObjectAck(_) => "create-object-ack",
            Self::DeleteObject(_) => "delete-object",
            Self::Error { .. } => "error",
            Self::CreateObjectError(_) => "create-object-error",
            Self::WriteMultipleError(_) => "write-property-multiple-error",
            Self::Undecoded => "undecoded",
        }
    }
}
