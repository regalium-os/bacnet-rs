// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Services (Clauses 13–17): what a request asks and what its answer says.
//!
//! Each service is a struct with `decode` and `encode` for its body — the
//! octets after the APDU's service choice. [`Service::decode`] reads an
//! [`Apdu`](crate::apdu::Apdu) as whichever of them it carries.
//!
//! # Built so far
//!
//! Discovery: Who-Is, I-Am, Who-Has, I-Have (§16.1, §16.8–§16.10). Reading:
//! `ReadProperty` and `ReadPropertyMultiple` with their ACKs (§15.5, §15.7).
//! Writing: `WriteProperty` and `WritePropertyMultiple` (§15.9, §15.10).
//! Change of value: `SubscribeCOV`, `SubscribeCOVProperty` and both COV
//! notifications (§13.6–§13.15). Objects: `CreateObject` and `DeleteObject`
//! (§15.3, §15.4). Alarms: both event notifications (§13.8, §13.9). Errors:
//! the plain one (§18) and the two structured ones. The rest of Clauses
//! 13–17 decode to
//! [`Service::Undecoded`] until the roadmap reaches them; their octets stay
//! in the APDU.
//!
//! # Lossless, one layer down
//!
//! A service is a reading of the tags beneath it and re-encodes
//! canonically: a device that wrote an instance in four octets where two
//! would do is answered in two. The octets as sent are never lost — the
//! APDU that carried them keeps them, and the tag layer reproduces them —
//! and a value whose type only its property knows stays undecoded octets,
//! as [`ReadPropertyAck::value`] does.

mod cov;
mod dispatch;
mod event;
mod fields;
mod i_am;
mod objects;
mod read_property;
mod reference;
mod rpm;
mod service_error;
mod traced;
mod values;
mod who_has;
mod who_is;
mod write_property;

pub use cov::{CovNotification, SubscribeCov, SubscribeCovProperty};
pub use dispatch::Service;
pub use event::{EventNotification, TimeStamp};
pub use i_am::IAm;
pub use objects::{CreateObject, CreateObjectError, ObjectBody, ObjectSpecifier};
pub use read_property::{ReadProperty, ReadPropertyAck};
pub use reference::{ObjectPropertyReference, PropertyReference, PropertyValue};
pub use rpm::{
    PropertyResult, ReadAccess, ReadPropertyMultiple, ReadPropertyMultipleAck, ReadResult,
};
pub use service_error::ServiceError;
pub use traced::decode_traced;
pub use who_has::{IHave, Sought, WhoHas};
pub use who_is::WhoIs;
pub use write_property::{WriteAccess, WriteMultipleError, WriteProperty, WritePropertyMultiple};

/// Unconfirmed service choices (§21, `BACnetUnconfirmedServiceChoice`).
pub mod unconfirmed {
    /// I-Am.
    pub const I_AM: u8 = 0;
    /// I-Have.
    pub const I_HAVE: u8 = 1;
    /// `UnconfirmedCOVNotification`.
    pub const COV_NOTIFICATION: u8 = 2;
    /// `UnconfirmedEventNotification`.
    pub const EVENT_NOTIFICATION: u8 = 3;
    /// Who-Has.
    pub const WHO_HAS: u8 = 7;
    /// Who-Is.
    pub const WHO_IS: u8 = 8;
}

/// Confirmed service choices (§21, `BACnetConfirmedServiceChoice`).
pub mod confirmed {
    /// `ConfirmedCOVNotification`.
    pub const COV_NOTIFICATION: u8 = 1;
    /// `ConfirmedEventNotification`.
    pub const EVENT_NOTIFICATION: u8 = 2;
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
    /// `SubscribeCOVProperty`.
    pub const SUBSCRIBE_COV_PROPERTY: u8 = 28;
}
