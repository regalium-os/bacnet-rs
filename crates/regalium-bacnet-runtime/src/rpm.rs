// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `ReadPropertyMultiple` (§15.7), with the fallback the standard allows a
//! device to need: not every device implements it, and one that says so is
//! read property by property from then on, without being asked again.

use crate::batch::Call;
use crate::client::Client;
use crate::error::{Error, RequestError};
use crate::read::ReadRequest;
use crate::reading::Reading;
use regalium_bacnet_core::datalink::Datalink;
use regalium_bacnet_core::service::{
    ReadAccess, ReadProperty, ReadPropertyMultiple, ReadPropertyMultipleAck, ServiceError,
    confirmed,
};

/// Property identifiers that ask for many properties at once (§15.7.1.1.2):
/// `all`, `required`, `optional`. Only `ReadPropertyMultiple` can.
const MANY: [u32; 3] = [8, 105, 80];

/// Reject reason unrecognized-service (§20.1.8.3).
const UNRECOGNIZED_SERVICE: u8 = 9;

/// Error class services, code optional-functionality-not-supported.
const NOT_SUPPORTED: ServiceError = ServiceError { class: 5, code: 45 };

impl<D: Datalink> Client<D> {
    /// Reads many properties of many objects of one device in one request,
    /// returning a result per property: in the order asked, or for `all`,
    /// `required` and `optional`, in the device's order.
    ///
    /// A device that does not implement `ReadPropertyMultiple` is read with
    /// `ReadProperty` instead, and remembered; `all`, `required` and
    /// `optional` then come back [`RequestError::Unsupported`].
    ///
    /// # Errors
    ///
    /// [`Error::Request`] when the whole request failed — the device did
    /// not answer, or refused it for another reason; [`Error::Link`] when
    /// the datalink failed.
    pub fn read_multiple(
        &mut self,
        device: u32,
        reads: &[ReadAccess],
    ) -> Result<Vec<Result<Reading, RequestError>>, Error<D::Error>> {
        if !self.no_rpm.contains(&device) {
            let mut body = Vec::new();
            ReadPropertyMultiple {
                reads: reads.to_vec(),
            }
            .encode(&mut body);
            let call = Call {
                device,
                service: confirmed::READ_PROPERTY_MULTIPLE,
                body,
            };
            let mut answers = self
                .transact(&[call], "bacnet.runtime.read_multiple")
                .map_err(Error::Link)?;
            match answers.pop().unwrap_or(Err(RequestError::Unanswered)) {
                Ok(Some(body)) => return flatten(device, &body).map_err(Error::Request),
                Ok(None) => return Err(Error::Request(RequestError::Malformed(Vec::new()))),
                Err(
                    RequestError::Reject(UNRECOGNIZED_SERVICE) | RequestError::Error(NOT_SUPPORTED),
                ) => {
                    self.no_rpm.insert(device);
                }
                Err(e) => return Err(Error::Request(e)),
            }
        }
        self.read_singly(device, reads).map_err(Error::Link)
    }

    /// Whether `device` is still asked with `ReadPropertyMultiple`: true
    /// until it refuses the service once.
    #[must_use]
    pub fn supports_read_multiple(&self, device: u32) -> bool {
        !self.no_rpm.contains(&device)
    }

    fn read_singly(
        &mut self,
        device: u32,
        reads: &[ReadAccess],
    ) -> Result<Vec<Result<Reading, RequestError>>, D::Error> {
        let wanted: Vec<ReadProperty> = reads
            .iter()
            .flat_map(|r| {
                r.properties.iter().map(|p| ReadProperty {
                    object: r.object,
                    property: p.property,
                    index: p.index,
                })
            })
            .collect();
        let singles: Vec<ReadRequest> = wanted
            .iter()
            .filter(|r| !MANY.contains(&r.property))
            .map(|r| ReadRequest { device, read: *r })
            .collect();
        let mut answers = self.read_many(&singles)?.into_iter();
        Ok(wanted
            .iter()
            .map(|r| {
                if MANY.contains(&r.property) {
                    Err(RequestError::Unsupported)
                } else {
                    answers.next().unwrap_or(Err(RequestError::Unanswered))
                }
            })
            .collect())
    }
}

/// An ACK as a result per property.
fn flatten(device: u32, body: &[u8]) -> Result<Vec<Result<Reading, RequestError>>, RequestError> {
    let ack = ReadPropertyMultipleAck::decode(body)
        .map_err(|_| RequestError::Malformed(body.to_vec()))?;
    Ok(ack
        .objects
        .iter()
        .flat_map(|o| {
            o.results.iter().map(move |r| {
                let read = ReadProperty {
                    object: o.object,
                    property: r.property.property,
                    index: r.property.index,
                };
                match r.result {
                    Ok(value) => Ok(Reading {
                        device,
                        read,
                        value: value.to_vec(),
                    }),
                    Err(e) => Err(RequestError::Error(e)),
                }
            })
        })
        .collect())
}
