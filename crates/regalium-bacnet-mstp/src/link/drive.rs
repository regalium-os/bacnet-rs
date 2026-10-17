// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Driving the master: its deadlines, the frames read, and what it asks to
//! send or hand up.

use super::{Line, Mstp};
use crate::frame::kind;
use crate::master::{Action, State};
use regalium_bacnet_core::datalink::Received;
use regalium_bacnet_telemetry::{Attribute, Span, keys};
use std::io::{self, ErrorKind};
use std::time::{Duration, Instant};

/// Octets read at a time: a few frames at the fastest baud rate.
const CHUNK: usize = 512;

impl<L: Line> Mstp<L> {
    pub(super) fn receive_until(
        &self,
        buf: &mut [u8],
        deadline: Instant,
    ) -> io::Result<Option<Received<u8>>> {
        let mut chunk = [0u8; CHUNK];
        loop {
            if let Some((from, npdu, broadcast)) = self.lock().inbox.pop_front() {
                let Some(into) = buf.get_mut(..npdu.len()) else {
                    return Err(io::Error::new(
                        ErrorKind::InvalidInput,
                        "receive buffer smaller than the NPDU",
                    ));
                };
                into.copy_from_slice(&npdu);
                let span = Span::start(&*self.tracer, "bacnet.mstp.receive");
                span.set(Attribute::int(
                    keys::PDU_OCTETS,
                    i64::try_from(npdu.len()).unwrap_or(i64::MAX),
                ));
                return Ok(Some(Received {
                    from,
                    len: npdu.len(),
                    broadcast,
                }));
            }
            let now = self.now();
            let (due, recovering, wait_master) = {
                let mut inner = self.lock();
                let recovering =
                    inner.master.state() == State::NoToken && now >= inner.master.deadline();
                let due = inner.master.poll(now);
                (due, recovering, inner.master.deadline().saturating_sub(now))
            };
            if recovering {
                let _span = Span::start(&*self.tracer, "bacnet.mstp.token.recover");
            }
            self.act(due)?;
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() && self.lock().inbox.is_empty() {
                return Ok(None);
            }
            let wait = left.min(wait_master).max(Duration::from_millis(1));
            let n = self
                .line
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .read(&mut chunk, wait)?;
            let Some(read) = chunk.get(..n) else { continue };
            if read.is_empty() {
                continue;
            }
            let now = self.now();
            let actions = {
                let mut inner = self.lock();
                let before = inner.reader.discarded();
                let frames = inner.reader.push(read);
                if inner.reader.discarded() > before {
                    Span::start(&*self.tracer, "bacnet.mstp.frame.decode")
                        .fail(&"a frame failed its check");
                }
                frames
                    .iter()
                    .flat_map(|f| inner.master.received(f, now))
                    .collect()
            };
            self.act(actions)?;
        }
    }

    /// Sends what the master asks to, and keeps what it hands up.
    pub(super) fn act(&self, actions: Vec<Action>) -> io::Result<()> {
        for action in actions {
            match action {
                Action::Send(frame) => {
                    let name = if frame.kind == kind::TOKEN {
                        "bacnet.mstp.token.pass"
                    } else {
                        "bacnet.mstp.send"
                    };
                    let span = Span::start(&*self.tracer, name);
                    let octets = frame.encode();
                    span.set(Attribute::int(
                        keys::PDU_OCTETS,
                        i64::try_from(octets.len()).unwrap_or(i64::MAX),
                    ));
                    let mut line = self
                        .line
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if let Err(e) = line.write(&octets) {
                        span.fail(&e);
                        return Err(e);
                    }
                }
                Action::Deliver {
                    source,
                    npdu,
                    broadcast,
                } => self.lock().inbox.push_back((source, npdu, broadcast)),
            }
        }
        Ok(())
    }
}
