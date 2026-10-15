// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The confirmed-request loop every service shares: send, retry, match the
//! reply by invoke id, and keep whatever else arrives meanwhile.

use super::{Client, Device, ReadError};
use crate::frame::{self, Delivery, Incoming};
use crate::service::{self, confirmed, cov, unconfirmed};
use crate::{metrics, names};
use regalium_bacnet_telemetry::{Attribute, keys};
use std::io;
use std::time::Instant;

impl Client {
    fn next_invoke(&mut self) -> u8 {
        self.invoke = self.invoke.wrapping_add(1);
        self.invoke
    }

    /// Sends a confirmed request and returns the Complex-ACK body, or an
    /// empty body for a Simple-ACK. Records `bacnet.client.requests` and
    /// `bacnet.client.request.duration`.
    pub(crate) fn confirm(
        &mut self,
        device: &Device,
        service: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ReadError> {
        let _ = self.maintain();
        let invoke = self.next_invoke();
        let apdu = service::confirmed_request(invoke, service, body);
        let packet = frame::encode(Delivery::Unicast, device.route.as_ref(), true, &apdu);
        let started = Instant::now();
        let mut result = Err(ReadError::Timeout);
        for _ in 0..=self.retries {
            if let Err(e) = self.send(&packet, device.addr) {
                result = Err(e.into());
                break;
            }
            match self.await_reply(device, invoke) {
                Err(ReadError::Timeout) => {}
                other => {
                    result = other;
                    break;
                }
            }
        }
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

    fn await_reply(&mut self, device: &Device, invoke: u8) -> Result<Vec<u8>, ReadError> {
        let deadline = Instant::now() + self.timeout;
        while let Some((datagram, peer)) = self.recv_until(deadline)? {
            let Some(msg) = frame::decode(&datagram, peer) else {
                continue;
            };
            if self.unsolicited(&msg)? || msg.from != device.addr || msg.source != device.route {
                continue;
            }
            let (Some(&pdu), Some(&id)) = (msg.apdu.first(), msg.apdu.get(1)) else {
                continue;
            };
            let reason = msg.apdu.get(2).copied().unwrap_or_default();
            let rest = || msg.apdu.get(3..).unwrap_or_default().to_vec();
            match pdu >> 4 {
                _ if id != invoke => {}
                2 => return Ok(Vec::new()),
                3 if pdu & 0x08 != 0 => {
                    // Abort our side politely so the device stops resending segments.
                    let abort = frame::encode(
                        Delivery::Unicast,
                        device.route.as_ref(),
                        false,
                        &[0x70, invoke, 4],
                    );
                    self.send(&abort, device.addr)?;
                    return Err(ReadError::Segmented);
                }
                3 => return Ok(rest()),
                5 => return Err(error_pdu(&rest())),
                6 => return Err(ReadError::Reject(reason)),
                7 => return Err(ReadError::Abort(reason)),
                _ => {}
            }
        }
        Err(ReadError::Timeout)
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

fn error_pdu(body: &[u8]) -> ReadError {
    use crate::tag::{Value, decode};
    let class = decode(body);
    let code = class
        .as_ref()
        .and_then(|(_, used)| decode(body.get(*used..)?));
    match (class, code) {
        (Some((Value::Enumerated(class), _)), Some((Value::Enumerated(code), _))) => {
            ReadError::Bacnet { class, code }
        }
        _ => ReadError::Malformed,
    }
}
