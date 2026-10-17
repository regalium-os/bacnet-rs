// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Confirmed requests, many in flight: the engine every service call runs
//! on — reads, writes, subscriptions.

use crate::client::Client;
use crate::error::RequestError;
use regalium_bacnet_core::datalink::Datalink;
use regalium_bacnet_core::service::ServiceError;
use regalium_bacnet_core::tsm::{Abandoned, Refused, Reply, Request};
use regalium_bacnet_telemetry::{Attribute, Span, keys};
use std::collections::BTreeMap;
use std::time::Duration;

/// One confirmed request to make.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Call {
    pub(crate) device: u32,
    pub(crate) service: u8,
    pub(crate) body: Vec<u8>,
}

/// What a confirmed request came back with: the body of a Complex-ACK, or
/// nothing for a Simple-ACK.
pub(crate) type Answer = Result<Option<Vec<u8>>, RequestError>;

/// How long a batch waits in all before it stops collecting. Every request
/// ends sooner, answered or abandoned by its own timers; this only bounds a
/// clock that jumped.
const BATCH_LIMIT: Duration = Duration::from_secs(3600);

impl<D: Datalink> Client<D> {
    /// Makes every call, keeping up to [`Config::in_flight`](crate::Config::in_flight)
    /// outstanding at once, inside a span named `span`, and returns each
    /// answer in the order asked.
    ///
    /// # Errors
    ///
    /// The datalink's, which ends the batch; each call's own failure is in
    /// its place in the list.
    pub(crate) fn transact(
        &mut self,
        calls: &[Call],
        name: &'static str,
    ) -> Result<Vec<Answer>, D::Error> {
        // The span outlives calls that borrow the client mutably.
        let tracer = self.tracer.clone();
        let span = Span::start(&*tracer, name);
        span.set(Attribute::int(
            keys::DATAGRAMS,
            i64::try_from(calls.len()).unwrap_or(i64::MAX),
        ));
        let mut answers: Vec<Option<Answer>> = vec![None; calls.len()];
        let mut pending = calls.iter().enumerate().peekable();
        let mut flying: BTreeMap<(u32, u8), usize> = BTreeMap::new();
        let deadline = self.now().saturating_add(BATCH_LIMIT);
        loop {
            while flying.len() < self.config.in_flight {
                let Some(&(i, call)) = pending.peek() else {
                    break;
                };
                let Some(device) = self.devices.get(&call.device) else {
                    put(&mut answers, i, Err(RequestError::NotBound));
                    pending.next();
                    continue;
                };
                let remote = device.remote(self.link.max_apdu());
                let request = Request::new(call.service, call.body.clone())
                    .remote(remote)
                    .within(span.id());
                match self.tsm.request(call.device, request, self.now()) {
                    Ok(invoke) => {
                        flying.insert((call.device, invoke), i);
                    }
                    Err(Refused::NoInvokeId) => break,
                    Err(e) => put(&mut answers, i, Err(RequestError::Refused(e))),
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
                put(&mut answers, i, answer(end));
            }
            if self.now() >= deadline {
                break;
            }
        }
        Ok(answers
            .into_iter()
            .map(|a| a.unwrap_or(Err(RequestError::Unanswered)))
            .collect())
    }
}

fn put(answers: &mut [Option<Answer>], i: usize, a: Answer) {
    if let Some(slot) = answers.get_mut(i) {
        *slot = Some(a);
    }
}

/// What a finished transaction means for its caller.
fn answer(end: Result<Reply, Abandoned>) -> Answer {
    match end {
        Ok(Reply::Simple) => Ok(None),
        Ok(Reply::Complex(body)) => Ok(Some(body)),
        Ok(Reply::Error(body)) => match ServiceError::decode(&body) {
            Ok(e) => Err(RequestError::Error(e)),
            Err(_) => Err(RequestError::Malformed(body.clone())),
        },
        Ok(Reply::Reject(r)) => Err(RequestError::Reject(r)),
        Ok(Reply::Abort(r)) => Err(RequestError::Abort(r)),
        Err(reason) => Err(RequestError::Offline(reason)),
    }
}
