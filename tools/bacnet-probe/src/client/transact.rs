// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The confirmed-request loop every service shares, driven by the driver's
//! transaction state machine: invoke ids, retries, segmented answers
//! reassembled, and whatever else arrives meanwhile kept.

use super::{Client, Device, ReadError};
use crate::frame::{self, Delivery, Incoming};
use crate::service::{self, confirmed, cov, unconfirmed};
use crate::{metrics, names};
use regalium_bacnet::apdu::Apdu;
use regalium_bacnet::tsm::{self, Abandoned, Remote, Reply, Request, Step};
use regalium_bacnet_telemetry::{Attribute, keys};
use std::io;
use std::time::{Duration, Instant};

/// The largest APDU BACnet/IP carries, and so the most a device behind it
/// can be sent whatever it claims.
const LINK_MAX_APDU: u16 = 1476;

impl Client {
    pub(crate) fn confirm(
        &mut self,
        device: &Device,
        service: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ReadError> {
        let _ = self.maintain();
        let started = Instant::now();
        let (result, invoke, attempts) = self.transact(device, service, body);
        self.set(Attribute::int(keys::INVOKE_ID, invoke.into()));
        self.set(Attribute::int(keys::ATTEMPTS, attempts.into()));
        let outcome = result.as_ref().map_or_else(ReadError::outcome, |_| "ok");
        let service = names::confirmed_service(service);
        let instance = i64::from(device.instance);
        let labels = [
            Attribute::str(keys::SERVICE, service),
            Attribute::str(keys::OUTCOME, outcome),
            Attribute::int(keys::DEVICE_INSTANCE, instance),
        ];
        self.meter.record(&metrics::REQUESTS, 1.0, &labels);
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        let labels = [
            Attribute::str(keys::SERVICE, service),
            Attribute::int(keys::DEVICE_INSTANCE, instance),
        ];
        self.meter
            .record(&metrics::REQUEST_DURATION, elapsed, &labels);
        result
    }

    /// One request through the state machine, to its end: the result, the
    /// invoke id it went under, and how many times it was sent.
    fn transact(
        &mut self,
        device: &Device,
        service: u8,
        body: &[u8],
    ) -> (Result<Vec<u8>, ReadError>, u8, u8) {
        // The TUI edits the timeout and retries live; take them as they are.
        let timers = tsm::Config::default()
            .apdu_timeout(self.timeout)
            .segment_timeout(self.timeout)
            .retries(u8::try_from(self.retries).unwrap_or(u8::MAX));
        self.tsm.reconfigure(timers);
        let max_apdu =
            u16::try_from(device.max_apdu).map_or(LINK_MAX_APDU, |m| m.min(LINK_MAX_APDU));
        let request = Request::new(service, body.to_vec()).remote(Remote {
            max_apdu,
            ..Remote::default()
        });
        let Ok(invoke) = self.tsm.request(device.instance, request, self.clock()) else {
            return (Err(ReadError::Segmented), 0, 0);
        };
        let mine = (device.instance, invoke);
        let mut attempts = 1;
        loop {
            match self.tsm.poll(self.clock()) {
                Step::Transmit { apdu, attempt, .. } => {
                    attempts = attempts.max(attempt);
                    let request = apdu.first().is_some_and(|o| o >> 4 == 0);
                    let packet =
                        frame::encode(Delivery::Unicast, device.route.as_ref(), request, &apdu);
                    if let Err(e) = self.send(&packet, device.addr) {
                        return (Err(e.into()), invoke, attempts);
                    }
                }
                Step::Complete {
                    peer,
                    invoke: i,
                    reply,
                } if (peer, i) == mine => {
                    return (answer(reply), invoke, attempts);
                }
                Step::Abandon {
                    peer,
                    invoke: i,
                    reason,
                } if (peer, i) == mine => {
                    let e = match reason {
                        Abandoned::Aborted(r) => ReadError::Abort(r),
                        Abandoned::Timeout { .. } | Abandoned::SegmentTimeout => ReadError::Timeout,
                    };
                    return (Err(e), invoke, attempts);
                }
                // The end of a request an earlier call gave up on mid-way.
                Step::Complete { .. } | Step::Abandon { .. } => {}
                Step::Idle { wake_at } => {
                    let wake = wake_at.unwrap_or_else(|| self.clock() + self.timeout);
                    if let Err(e) = self.hear(device, self.origin + wake) {
                        return (Err(e.into()), invoke, attempts);
                    }
                }
            }
        }
    }

    /// Waits until `deadline` for one datagram: a COV notification is kept,
    /// anything from `device` goes to the state machine, the rest dropped.
    fn hear(&mut self, device: &Device, deadline: Instant) -> io::Result<()> {
        let Some((datagram, peer)) = self.recv_until(deadline)? else {
            return Ok(());
        };
        let Some(msg) = frame::decode(&datagram, peer) else {
            return Ok(());
        };
        if self.unsolicited(&msg)? || msg.from != device.addr || msg.source != device.route {
            return Ok(());
        }
        if let Ok(apdu) = Apdu::decode(msg.apdu) {
            let now = self.clock();
            self.tsm.receive(device.instance, &apdu, now);
        }
        Ok(())
    }

    /// Time since the client was bound: the clock its state machines run on.
    pub(crate) fn clock(&self) -> Duration {
        self.origin.elapsed()
    }

    /// Keeps a COV notification that arrived out of turn, acknowledging a
    /// confirmed one as §13.7 requires. True when `msg` was one.
    pub(crate) fn unsolicited(&mut self, msg: &Incoming<'_>) -> io::Result<bool> {
        let (body, ack) = match msg.apdu {
            [0x10, unconfirmed::COV_NOTIFICATION, body @ ..] => (body, None),
            [first, _, invoke, confirmed::COV_NOTIFICATION, body @ ..] if first >> 4 == 0 => {
                (body, Some(*invoke))
            }
            _ => return Ok(false),
        };
        if let Some(invoke) = ack {
            let reply = service::simple_ack(invoke, confirmed::COV_NOTIFICATION);
            self.send(
                &frame::encode(Delivery::Unicast, msg.source.as_ref(), false, &reply),
                msg.from,
            )?;
        }
        if let Some(n) = cov::decode_notification(body) {
            let labels = [Attribute::int(
                keys::DEVICE_INSTANCE,
                i64::from(n.device.instance),
            )];
            self.meter.record(&metrics::COV_NOTIFICATIONS, 1.0, &labels);
            self.pending.push_back(n);
        }
        Ok(true)
    }
}

/// What an answer means for a request.
fn answer(reply: Reply) -> Result<Vec<u8>, ReadError> {
    match reply {
        Reply::Simple => Ok(Vec::new()),
        Reply::Complex(body) => Ok(body),
        Reply::Error(body) => Err(error_pdu(&body)),
        Reply::Reject(r) => Err(ReadError::Reject(r)),
        Reply::Abort(r) => Err(ReadError::Abort(r)),
    }
}

fn error_pdu(body: &[u8]) -> ReadError {
    crate::service::object::decode_error(body).map_or(ReadError::Malformed, |(class, code)| {
        ReadError::Bacnet { class, code }
    })
}
