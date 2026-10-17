// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `WriteProperty` (§15.9).

use crate::batch::Call;
use crate::client::Client;
use crate::error::{Error, RequestError};
use regalium_bacnet_core::datalink::Datalink;
use regalium_bacnet_core::primitive::{ObjectIdentifier, Primitive};
use regalium_bacnet_core::service::{PropertyReference, PropertyValue, WriteProperty, confirmed};

impl<D: Datalink> Client<D> {
    /// Writes one value into `property` of `object` at `priority` (1–16,
    /// for a commandable property; `None` for the rest).
    ///
    /// A write the device acknowledges may still change nothing: a value
    /// held at a higher priority wins (§19.2). Read the priority array to
    /// know which slot holds the value.
    ///
    /// # Errors
    ///
    /// [`Error::Request`] when the device refused it or did not answer;
    /// [`Error::Link`] when the datalink failed.
    pub fn write_property(
        &mut self,
        device: u32,
        object: ObjectIdentifier,
        property: u32,
        value: &Primitive<'_>,
        priority: Option<u8>,
    ) -> Result<(), Error<D::Error>> {
        let mut encoded = Vec::new();
        value.encode(&mut encoded);
        self.write_encoded(
            device,
            object,
            PropertyReference::new(property),
            &encoded,
            priority,
        )
    }

    /// Writes a value already encoded as its elements — a constructed one,
    /// a list — into one property or array element.
    ///
    /// # Errors
    ///
    /// As [`Client::write_property`].
    pub fn write_encoded(
        &mut self,
        device: u32,
        object: ObjectIdentifier,
        property: PropertyReference,
        value: &[u8],
        priority: Option<u8>,
    ) -> Result<(), Error<D::Error>> {
        let mut body = Vec::new();
        WriteProperty {
            object,
            value: PropertyValue {
                property,
                value,
                priority,
            },
        }
        .encode(&mut body);
        let call = Call {
            device,
            service: confirmed::WRITE_PROPERTY,
            body,
        };
        let mut answers = self
            .transact(&[call], "bacnet.runtime.write")
            .map_err(Error::Link)?;
        match answers.pop().unwrap_or(Err(RequestError::Unanswered)) {
            Ok(None) => Ok(()),
            Ok(Some(body)) => Err(Error::Request(RequestError::Malformed(body))),
            Err(e) => Err(Error::Request(e)),
        }
    }
}
