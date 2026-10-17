// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A value read, owned, and its place in the protobuf model.

use crate::error::RequestError;
use regalium_bacnet_core::primitive::Primitive;
use regalium_bacnet_core::service::{ReadProperty, ReadPropertyAck};
use regalium_bacnet_schema::convert;
use regalium_bacnet_schema::object::Property;

/// What a `ReadProperty-ACK` said, owned so it outlives the datagram.
///
/// Plain data: `Send` and `Sync`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Reading {
    /// The device it came from.
    pub device: u32,
    /// What was read, as the device echoed it.
    pub read: ReadProperty,
    /// The value's octets, as sent.
    pub value: Vec<u8>,
}

impl Reading {
    /// The value as one primitive, when it is one: a present-value, a name.
    #[must_use]
    pub fn primitive(&self) -> Option<Primitive<'_>> {
        self.ack().primitive()
    }

    /// The reading as the model's [`Property`], named
    /// `devices/{device}/objects/{object}/properties/{property}`, its value
    /// typed where the tags say and its octets always kept.
    ///
    /// # Errors
    ///
    /// [`RequestError::Malformed`], carrying the octets, when the value is not
    /// well-formed elements. Owned, so it outlives the reading.
    pub fn property(&self) -> Result<Property, RequestError> {
        convert::property(self.device, &self.ack())
            .map_err(|_| RequestError::Malformed(self.value.clone()))
    }

    fn ack(&self) -> ReadPropertyAck<'_> {
        ReadPropertyAck {
            read: self.read,
            value: &self.value,
        }
    }
}
