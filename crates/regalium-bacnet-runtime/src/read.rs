// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `ReadProperty` (§15.5), one at a time or many in flight.

use crate::client::Client;
use crate::error::{Error, ReadError};
use crate::reading::Reading;
use regalium_bacnet_core::datalink::Datalink;
use regalium_bacnet_core::primitive::ObjectIdentifier;
use regalium_bacnet_core::service::{ReadProperty, ReadPropertyAck, ServiceError, confirmed};
use regalium_bacnet_core::tsm::{Refused, Reply, Request};
use regalium_bacnet_telemetry::{Attribute, Span, keys};
use std::collections::BTreeMap;
use std::time::Duration;

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

/// How long a batch waits in all before it stops collecting. Every request
/// ends sooner, answered or abandoned by its own timers; this only bounds a
/// clock that jumped.
const BATCH_LIMIT: Duration = Duration::from_secs(3600);

impl<D: Datalink> Client<D> {
    /// Reads one property.
    ///
    /// # Errors
    ///
    /// [`Error::Read`] for what the device or the network did — refused,
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
            .unwrap_or(Err(ReadError::NotBound))
            .map_err(Error::Read)
    }

    /// Reads every request, keeping up to [`Config::in_flight`](crate::Config::in_flight)
    /// outstanding at once, and returns each result in the order asked.
    ///
    /// # Errors
    ///
    /// The datalink's, which ends the batch; each read's own failure is in
    /// its place in the list.
    pub fn read_many(
        &mut self,
        requests: &[ReadRequest],
    ) -> Result<Vec<Result<Reading, ReadError>>, D::Error> {
        // The span outlives calls that borrow the client mutably.
        let tracer = self.tracer.clone();
        let span = Span::start(&*tracer, "bacnet.runtime.read");
        span.set(Attribute::int(
            keys::DATAGRAMS,
            i64::try_from(requests.len()).unwrap_or(i64::MAX),
        ));
        let mut results: Vec<Option<Result<Reading, ReadError>>> = vec![None; requests.len()];
        let mut pending = requests.iter().enumerate().peekable();
        let mut flying: BTreeMap<(u32, u8), usize> = BTreeMap::new();
        let deadline = self.now().saturating_add(BATCH_LIMIT);
        loop {
            while flying.len() < self.config.in_flight {
                let Some(&(i, r)) = pending.peek() else { break };
                let Some(device) = self.devices.get(&r.device) else {
                    put(&mut results, i, Err(ReadError::NotBound));
                    pending.next();
                    continue;
                };
                let mut body = Vec::new();
                r.read.encode(&mut body);
                let remote = device.remote(self.link.max_apdu());
                let request = Request::new(confirmed::READ_PROPERTY, body)
                    .remote(remote)
                    .within(span.id());
                match self.tsm.request(r.device, request, self.now()) {
                    Ok(invoke) => {
                        flying.insert((r.device, invoke), i);
                    }
                    Err(Refused::NoInvokeId) => break,
                    Err(e) => put(&mut results, i, Err(ReadError::Refused(e))),
                }
                pending.next();
            }
            if flying.is_empty() && pending.peek().is_none() {
                break;
            }
            self.pump(deadline, |c| flying.keys().any(|k| c.done.contains_key(k)))?;
            let finished: Vec<_> = flying
                .keys()
                .filter(|k| self.done.contains_key(k))
                .copied()
                .collect();
            for key in finished {
                let (Some(i), Some(end)) = (flying.remove(&key), self.done.remove(&key)) else {
                    continue;
                };
                put(&mut results, i, answer(key.0, end));
            }
            if self.now() >= deadline {
                break;
            }
        }
        Ok(results
            .into_iter()
            .map(|r| r.unwrap_or(Err(ReadError::Unanswered)))
            .collect())
    }
}

fn put(
    results: &mut [Option<Result<Reading, ReadError>>],
    i: usize,
    r: Result<Reading, ReadError>,
) {
    if let Some(slot) = results.get_mut(i) {
        *slot = Some(r);
    }
}

/// What a finished transaction means for a read.
fn answer(
    device: u32,
    end: Result<Reply, regalium_bacnet_core::tsm::Abandoned>,
) -> Result<Reading, ReadError> {
    match end {
        Ok(Reply::Complex(body)) => match ReadPropertyAck::decode(&body) {
            Ok(ack) => Ok(Reading {
                device,
                read: ack.read,
                value: ack.value.to_vec(),
            }),
            Err(_) => Err(ReadError::Malformed(body)),
        },
        Ok(Reply::Error(body)) => ServiceError::decode(&body).map_or_else(
            |_| Err(ReadError::Malformed(body.clone())),
            |e| Err(ReadError::Error(e)),
        ),
        Ok(Reply::Simple) => Err(ReadError::Malformed(Vec::new())),
        Ok(Reply::Reject(r)) => Err(ReadError::Reject(r)),
        Ok(Reply::Abort(r)) => Err(ReadError::Abort(r)),
        Err(reason) => Err(ReadError::Offline(reason)),
    }
}
