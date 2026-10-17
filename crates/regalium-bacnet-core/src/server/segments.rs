// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Sending a segmented answer (§5.4.5.2–§5.4.5.3): the first segment alone,
//! then windows as large as the requester's Segment-ACKs allow, each
//! window sent again when its acknowledgement does not come.

use super::outgoing::{Outgoing, seq};
use super::{Response, abort, answer, respond};
use crate::apdu::{Apdu, Pdu, Segments};
use crate::obj::Database;
use crate::tsm::Config;
use core::time::Duration;
use std::collections::BTreeMap;

/// Octets of a segmented Complex-ACK's header: type, invoke id, sequence,
/// window, service.
const HEADER: usize = 5;
/// Abort reason buffer-overflow: more segments than the requester takes.
const BUFFER_OVERFLOW: u8 = 1;

/// A server's answers, with the state to segment the large ones. `P` names
/// the requester as the runtime sends to it.
///
/// Plain data: `Send` and `Sync` when `P` is. One task owns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Responder<P> {
    config: Config,
    sending: BTreeMap<(P, u8), Outgoing>,
}

impl<P: Clone + Ord> Responder<P> {
    /// A responder using `config`'s segment timeout, retries and window.
    #[must_use]
    pub const fn new(config: Config) -> Self {
        Self {
            config,
            sending: BTreeMap::new(),
        }
    }

    /// The answers `apdu` from `peer` calls for at `now`: the reply or
    /// broadcast [`respond`] gives, the first segment of a large answer, or
    /// the next window when it is a Segment-ACK.
    pub fn handle(
        &mut self,
        peer: &P,
        apdu: &Apdu<'_>,
        db: &mut dyn Database,
        now: Duration,
    ) -> Vec<Response> {
        match apdu.pdu {
            Pdu::SegmentAck {
                invoke_id,
                sequence,
                window,
            } if !apdu.from_server() => {
                self.acked(peer, invoke_id, sequence, window, apdu.is_negative(), now)
            }
            Pdu::Abort { invoke_id, .. } if !apdu.from_server() => {
                self.sending.remove(&(peer.clone(), invoke_id));
                Vec::new()
            }
            Pdu::ConfirmedRequest {
                invoke_id,
                service,
                limits,
                segment: None,
                body,
            } => {
                let reply = answer(invoke_id, service, body, db);
                let max = usize::from(limits.max_apdu().unwrap_or(50));
                if reply.len() <= max {
                    return vec![Response::Reply(reply)];
                }
                let (Some(&0x30), Some(content)) = (reply.first(), reply.get(3..)) else {
                    return vec![abort(invoke_id)];
                };
                let size = max.saturating_sub(HEADER);
                if !apdu.accepts_segmented_response() || size == 0 {
                    return vec![abort(invoke_id)];
                }
                let count = content.len().div_ceil(size);
                if let Segments::AtMost(n) = limits.max_segments()
                    && count > usize::from(n)
                {
                    return vec![Response::Reply(vec![0x71, invoke_id, BUFFER_OVERFLOW])];
                }
                let mut o = Outgoing {
                    service,
                    body: content.to_vec(),
                    size,
                    count,
                    base: 0,
                    next: 0,
                    window: 1,
                    retries: 0,
                    deadline: now.saturating_add(self.config.segment_timeout),
                };
                let out = o.fill(invoke_id, self.config.window);
                self.sending.insert((peer.clone(), invoke_id), o);
                out
            }
            _ => respond(apdu, db).into_iter().collect(),
        }
    }

    /// Windows to send again at `now`, to whom; a send whose retries are
    /// spent is dropped, as §5.4.5.3 has the responder give up silently.
    pub fn poll(&mut self, now: Duration) -> Vec<(P, Response)> {
        let due: Vec<_> = self
            .sending
            .iter()
            .filter(|(_, o)| o.deadline <= now)
            .map(|(k, _)| k.clone())
            .collect();
        let mut out = Vec::new();
        for key in due {
            let Some(mut o) = self.sending.remove(&key) else {
                continue;
            };
            if o.retries >= self.config.retries {
                continue;
            }
            o.retries += 1;
            o.next = o.base;
            o.deadline = now.saturating_add(self.config.segment_timeout);
            out.extend(
                o.fill(key.1, self.config.window)
                    .into_iter()
                    .map(|r| (key.0.clone(), r)),
            );
            self.sending.insert(key, o);
        }
        out
    }

    /// When [`Responder::poll`] next has something to do.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        self.sending.values().map(|o| o.deadline).min()
    }

    fn acked(
        &mut self,
        peer: &P,
        invoke: u8,
        sequence: u8,
        window: u8,
        nak: bool,
        now: Duration,
    ) -> Vec<Response> {
        let key = (peer.clone(), invoke);
        let Some(mut o) = self.sending.remove(&key) else {
            return Vec::new();
        };
        let from = if nak {
            o.base.saturating_sub(1)
        } else {
            o.base
        };
        o.deadline = now.saturating_add(self.config.segment_timeout);
        let Some(acked) = (from..o.next).find(|i| seq(*i) == sequence) else {
            self.sending.insert(key, o);
            return Vec::new();
        };
        if acked + 1 >= o.count {
            return Vec::new();
        }
        o.base = acked + 1;
        o.next = o.base;
        o.window = window.clamp(1, 127);
        o.retries = 0;
        let out = o.fill(invoke, self.config.window);
        self.sending.insert(key, o);
        out
    }
}
