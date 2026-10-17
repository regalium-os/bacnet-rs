// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `ReadProperty` (§15.5), one at a time or many in flight.

use crate::batch::{Answer, Call};
use crate::client::Client;
use crate::error::{Error, RequestError};
use crate::reading::Reading;
use regalium_bacnet_core::datalink::Datalink;
use regalium_bacnet_core::primitive::ObjectIdentifier;
use regalium_bacnet_core::service::{ReadProperty, ReadPropertyAck, confirmed};

/// One read to make: which device, and which property of which object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReadRequest {
    /// The device instance; it must be bound.
    pub device: u32,
    /// What to read.
    pub read: ReadProperty,
}

impl ReadRequest {
    /// The whole of `property` of `object` in `device`.
    #[must_use]
    pub const fn new(device: u32, object: ObjectIdentifier, property: u32) -> Self {
        Self {
            device,
            read: ReadProperty {
                object,
                property,
                index: None,
            },
        }
    }
}

impl<D: Datalink> Client<D> {
    /// Reads one property.
    ///
    /// # Errors
    ///
    /// [`Error::Request`] for what the device or the network did — refused,
    /// silent, not bound — and [`Error::Link`] when the datalink failed.
    pub fn read_property(
        &mut self,
        device: u32,
        object: ObjectIdentifier,
        property: u32,
    ) -> Result<Reading, Error<D::Error>> {
        let mut all = self
            .read_many(&[ReadRequest::new(device, object, property)])
            .map_err(Error::Link)?;
        all.pop()
            .unwrap_or(Err(RequestError::Unanswered))
            .map_err(Error::Request)
    }

    /// Reads every request, keeping several in flight, and returns each
    /// result in the order asked.
    ///
    /// # Errors
    ///
    /// The datalink's, which ends the batch; each read's own failure is in
    /// its place in the list.
    pub fn read_many(
        &mut self,
        requests: &[ReadRequest],
    ) -> Result<Vec<Result<Reading, RequestError>>, D::Error> {
        let calls: Vec<Call> = requests
            .iter()
            .map(|r| {
                let mut body = Vec::new();
                r.read.encode(&mut body);
                Call {
                    device: r.device,
                    service: confirmed::READ_PROPERTY,
                    body,
                }
            })
            .collect();
        let answers = self.transact(&calls, "bacnet.runtime.read")?;
        Ok(requests
            .iter()
            .zip(answers)
            .map(|(r, a)| reading(r.device, a))
            .collect())
    }
}

/// A `ReadProperty` answer as a reading.
fn reading(device: u32, answer: Answer) -> Result<Reading, RequestError> {
    let body = answer?.ok_or(RequestError::Malformed(Vec::new()))?;
    match ReadPropertyAck::decode(&body) {
        Ok(ack) => Ok(Reading {
            device,
            read: ack.read,
            value: ack.value.to_vec(),
        }),
        Err(_) => Err(RequestError::Malformed(body)),
    }
}
