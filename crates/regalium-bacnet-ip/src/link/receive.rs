// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Receiving: wait for a datagram until the caller's deadline, doing the
//! datalink's own housekeeping on the way, and hand up only NPDUs.

use super::{BacnetIp, Mode};
use crate::bbmd::Out;
use crate::bvll::{Bvll, Function};
use crate::foreign::Outcome;
use regalium_bacnet_core::datalink::Received;
use regalium_bacnet_telemetry::{Attribute, Span, keys};
use std::io::{self, ErrorKind};
use std::net::{SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};

/// Larger than any BACnet/IP datagram: an Ethernet frame's payload.
const DATAGRAM: usize = 1600;

impl BacnetIp {
    pub(super) fn receive_until(
        &self,
        buf: &mut [u8],
        deadline: Instant,
    ) -> io::Result<Option<Received<SocketAddrV4>>> {
        let mut datagram = [0u8; DATAGRAM];
        loop {
            self.housekeeping()?;
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            let mut wait = deadline - now;
            if let Some(due) = self.next_due() {
                wait = wait.min(due.saturating_sub(self.now()));
            }
            // A zero timeout means "block forever" to the socket.
            self.socket
                .set_read_timeout(Some(wait.max(Duration::from_millis(1))))?;
            let (len, from) = match self.socket.recv_from(&mut datagram) {
                Ok(got) => got,
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    continue;
                }
                Err(e) => return Err(e),
            };
            // Its own broadcasts come back to it; other families are not
            // BACnet/IP.
            let SocketAddr::V4(from) = from else { continue };
            if from == self.local {
                continue;
            }
            if let Some(got) = self.dispatch(from, datagram.get(..len).unwrap_or(&[]), buf)? {
                return Ok(Some(got));
            }
        }
    }

    /// A registration due, or foreign devices to forget.
    fn housekeeping(&self) -> io::Result<()> {
        let now = self.now();
        let send = match &mut *self.lock() {
            Mode::Foreign(r) => r.poll(now).map(|m| (m, r.bbmd())),
            Mode::Bbmd(b) => {
                b.expire(now);
                None
            }
            Mode::Plain => None,
        };
        match send {
            Some((message, bbmd)) => self.transmit(&message, bbmd),
            None => Ok(()),
        }
    }

    fn next_due(&self) -> Option<Duration> {
        match &*self.lock() {
            Mode::Foreign(r) => Some(r.deadline()),
            Mode::Bbmd(b) => b.deadline(),
            Mode::Plain => None,
        }
    }

    /// One datagram: anything that is not well-formed BACnet/IP is dropped,
    /// management is handled here, and an NPDU is copied into `buf`.
    fn dispatch(
        &self,
        from: SocketAddrV4,
        octets: &[u8],
        buf: &mut [u8],
    ) -> io::Result<Option<Received<SocketAddrV4>>> {
        let Ok(bvll) = Bvll::decode(octets) else {
            return Ok(None);
        };
        // The codec keeps a length field that disagrees with the datagram,
        // so the datagram can be inspected; a link does not act on one.
        if usize::from(bvll.length) != octets.len() {
            return Ok(None);
        }
        let forwarded = matches!(bvll.function, Function::Forwarded { .. });
        let span = Span::start(
            &*self.tracer,
            if forwarded {
                "bacnet.ip.forward"
            } else {
                "bacnet.ip.receive"
            },
        );
        span.set(Attribute::str(keys::BVLC_FUNCTION, bvll.name()));
        span.set(Attribute::int(
            keys::PDU_OCTETS,
            i64::try_from(octets.len()).unwrap_or(i64::MAX),
        ));
        let now = self.now();
        let (out, delivered): (Vec<Out>, _) = match &mut *self.lock() {
            Mode::Bbmd(b) => {
                let h = b.handle(from, &bvll, now);
                (h.out, h.deliver)
            }
            Mode::Foreign(r) => match bvll.function {
                Function::Result(code) => {
                    if let Some(outcome) = r.result(from, code, now) {
                        let s =
                            Span::within(&*self.tracer, "bacnet.ip.foreign.register", span.id());
                        let ok = matches!(outcome, Outcome::Registered { .. });
                        s.set(Attribute::str(
                            keys::OUTCOME,
                            if ok { "ok" } else { "refused" },
                        ));
                    }
                    (Vec::new(), None)
                }
                _ => (Vec::new(), npdu_sender(from, &bvll)),
            },
            Mode::Plain => (Vec::new(), npdu_sender(from, &bvll)),
        };
        if !out.is_empty() {
            let _distribute = Span::within(&*self.tracer, "bacnet.ip.bbmd.distribute", span.id());
            self.deliver(&out)?;
        }
        let (Some((sender, broadcast)), Some(npdu)) = (delivered, bvll.npdu()) else {
            return Ok(None);
        };
        let Some(into) = buf.get_mut(..npdu.len()) else {
            let e = io::Error::new(
                ErrorKind::InvalidInput,
                "receive buffer smaller than the NPDU",
            );
            span.fail(&e);
            return Err(e);
        };
        into.copy_from_slice(npdu);
        Ok(Some(Received {
            from: sender,
            len: npdu.len(),
            broadcast,
        }))
    }
}

/// Who an NPDU-carrying message is from, for a node that is not a BBMD: the
/// sender, or for a Forwarded-NPDU the station that first sent it.
fn npdu_sender(from: SocketAddrV4, bvll: &Bvll<'_>) -> Option<(SocketAddrV4, bool)> {
    match bvll.function {
        Function::OriginalUnicast(_) => Some((from, false)),
        Function::OriginalBroadcast(_) => Some((from, true)),
        Function::Forwarded { origin, .. } => Some((origin, true)),
        _ => None,
    }
}
