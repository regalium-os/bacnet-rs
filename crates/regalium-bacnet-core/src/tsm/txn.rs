// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! One open transaction, and the APDUs it sends: request segments,
//! Segment-ACKs and Aborts, built with the codec so they are exactly what
//! [`Apdu::decode`](crate::apdu::Apdu::decode) reads back.

use super::config::Config;
use super::step::Step;
use crate::apdu::{Apdu, Pdu, Segment};
use core::time::Duration;
use regalium_bacnet_telemetry::SpanId;
use std::collections::VecDeque;

/// Octet 0's bits for a request (§20.1.2.1).
const SEG: u8 = 0x08;
const MOR: u8 = 0x04;
const SA: u8 = 0x02;

/// Where a client transaction is (§5.4.4). Idle is the absence of one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum State {
    /// Sending a segmented request: `base` is the first segment not yet
    /// acknowledged, `next` the first not yet sent, `window` how many the
    /// server takes before acknowledging — 1 until it has said.
    SegmentedRequest {
        base: usize,
        next: usize,
        window: u8,
    },
    /// The request is out; waiting for the answer.
    AwaitConfirmation,
    /// Reassembling a segmented answer: `last` is the last segment received
    /// in order, `initial` the last one acknowledged.
    SegmentedConfirmation {
        last: u8,
        initial: u8,
        window: u8,
        data: Vec<u8>,
    },
}

/// An open transaction. Owns the request, so it can be sent again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Txn {
    pub(crate) service: u8,
    pub(crate) body: Vec<u8>,
    /// Service data octets per segment, and how many segments; 1 is an
    /// unsegmented request.
    pub(crate) size: usize,
    pub(crate) count: usize,
    pub(crate) state: State,
    pub(crate) deadline: Duration,
    /// Sends of the whole request, the first included.
    pub(crate) attempts: u8,
    /// Resends of one window while a segmented request makes no progress.
    pub(crate) window_retries: u8,
    pub(crate) span: SpanId,
}

impl Txn {
    /// The confirmed request, or segment `i` of it.
    pub(crate) fn request_apdu(&self, invoke: u8, i: usize, config: &Config) -> Vec<u8> {
        let sa = if config.segmented_response { SA } else { 0 };
        let (flags, segment, body) = if self.count <= 1 {
            (sa, None, self.body.as_slice())
        } else {
            let start = i.saturating_mul(self.size);
            let end = start.saturating_add(self.size).min(self.body.len());
            let more = if i + 1 < self.count { MOR } else { 0 };
            let segment = Segment {
                sequence: seq(i),
                window: config.window,
            };
            let body = self.body.get(start..end).unwrap_or(&[]);
            (SEG | more | sa, Some(segment), body)
        };
        encode(&Apdu {
            flags,
            pdu: Pdu::ConfirmedRequest {
                limits: config.accept,
                invoke_id: invoke,
                segment,
                service: self.service,
                body,
            },
            trailing: &[],
        })
    }

    /// Sends the segments the window allows from `next` on, and moves
    /// `next` past them. Nothing for an unsegmented request.
    pub(crate) fn fill<P: Copy>(
        &mut self,
        peer: P,
        invoke: u8,
        config: &Config,
        steps: &mut VecDeque<Step<P>>,
    ) {
        let State::SegmentedRequest { base, next, window } = self.state else {
            return;
        };
        let end = base.saturating_add(usize::from(window)).min(self.count);
        for i in next..end {
            steps.push_back(Step::Transmit {
                peer,
                invoke,
                attempt: self.attempts,
                apdu: self.request_apdu(invoke, i, config),
            });
        }
        self.state = State::SegmentedRequest {
            base,
            next: end.max(next),
            window,
        };
    }
}

/// A client's Segment-ACK (§20.1.6): `SRV` clear, `NAK` when asking for a
/// segment again.
pub(crate) fn segment_ack(invoke: u8, sequence: u8, window: u8, nak: bool) -> Vec<u8> {
    encode(&Apdu {
        flags: if nak { 0x02 } else { 0 },
        pdu: Pdu::SegmentAck {
            invoke_id: invoke,
            sequence,
            window,
        },
        trailing: &[],
    })
}

/// A client's Abort (§20.1.9), `SRV` clear.
pub(crate) fn abort(invoke: u8, reason: u8) -> Vec<u8> {
    encode(&Apdu {
        flags: 0,
        pdu: Pdu::Abort {
            invoke_id: invoke,
            reason,
        },
        trailing: &[],
    })
}

fn encode(apdu: &Apdu<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    apdu.encode(&mut out);
    out
}

/// A segment's sequence number: its index modulo 256 (§20.1.2.7).
pub(crate) fn seq(i: usize) -> u8 {
    u8::try_from(i & 0xFF).unwrap_or(0)
}
