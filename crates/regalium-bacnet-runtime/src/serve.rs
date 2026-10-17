// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The serve loop: receive a request, answer it — in segments when it is
//! large — and send again what a requester has not acknowledged.

use crate::device::Route;
use crate::server::Server;
use regalium_bacnet_core::apdu::Apdu;
use regalium_bacnet_core::datalink::{Datalink, Recipient};
use regalium_bacnet_core::npdu::{Address, Body, Destination, Npdu};
use regalium_bacnet_core::obj::Database;
use regalium_bacnet_core::server::Response;
use regalium_bacnet_telemetry::{Attribute, Span, keys};
use std::time::{Duration, Instant};

/// Larger than any NPDU a datalink carries.
const BUFFER: usize = 2048;

impl<D: Datalink, B: Database> Server<D, B> {
    /// Answers requests for `wait`.
    ///
    /// # Errors
    ///
    /// The datalink's. A malformed request is answered with a Reject, or
    /// dropped when it cannot be answered at all; neither is an error.
    pub fn serve(&mut self, wait: Duration) -> Result<(), D::Error> {
        let deadline = Instant::now() + wait;
        let mut buf = vec![0u8; BUFFER];
        loop {
            for (peer, r) in self.responder.poll(self.origin.elapsed()) {
                self.answer(&peer, r)?;
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(());
            }
            let wake = self
                .responder
                .deadline()
                .map_or(deadline, |d| deadline.min(self.origin + d));
            let Some(got) = self
                .link
                .receive(&mut buf, wake.saturating_duration_since(now))?
            else {
                continue;
            };
            let Some(octets) = buf.get(..got.len) else {
                continue;
            };
            let Ok(npdu) = Npdu::decode(octets) else {
                continue;
            };
            let Body::Apdu(body) = npdu.body else {
                continue;
            };
            let Ok(apdu) = Apdu::decode(body) else {
                continue;
            };
            let route = npdu.source.map(|s| Route {
                network: s.network,
                mac: s.mac.to_vec(),
            });
            let peer = (got.from, route);
            let responses =
                self.responder
                    .handle(&peer, &apdu, &mut self.db, self.origin.elapsed());
            if responses.is_empty() {
                continue;
            }
            let span = Span::start(&*self.tracer, "bacnet.runtime.serve");
            span.set(Attribute::int(
                keys::PDU_OCTETS,
                i64::try_from(got.len).unwrap_or(i64::MAX),
            ));
            for r in responses {
                self.answer(&peer, r)?;
            }
        }
    }

    /// Sends one answer: a broadcast to everyone, a reply back the way the
    /// request came — to the router, naming the requester's network, when
    /// it came through one.
    fn answer(
        &self,
        (to, route): &(D::Mac, Option<Route>),
        response: Response,
    ) -> Result<(), D::Error> {
        match response {
            Response::Broadcast(apdu) => self.broadcast(&apdu),
            Response::Reply(apdu) => {
                let destination = route.as_ref().map(|r| Destination {
                    address: Address {
                        network: r.network,
                        mac: &r.mac,
                    },
                    hop_count: 255,
                });
                let reply = Npdu {
                    control: 0,
                    destination,
                    source: None,
                    body: Body::Apdu(&apdu),
                };
                let mut out = Vec::new();
                reply.encode(&mut out);
                self.link.send(Recipient::Unicast(*to), &out)
            }
        }
    }
}
