// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! One segmented answer being sent: where it is, and the segments a window
//! allows next.

use super::Response;
use crate::apdu::{Apdu, Pdu, Segment};
use core::time::Duration;

/// A segmented answer being sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Outgoing {
    pub(super) service: u8,
    pub(super) body: Vec<u8>,
    pub(super) size: usize,
    pub(super) count: usize,
    /// First segment not acknowledged; first not sent; the window.
    pub(super) base: usize,
    pub(super) next: usize,
    pub(super) window: u8,
    pub(super) retries: u8,
    pub(super) deadline: Duration,
}

impl Outgoing {
    /// The segments the window allows from `next` on.
    pub(super) fn fill(&mut self, invoke: u8, proposed: u8) -> Vec<Response> {
        let end = self
            .base
            .saturating_add(usize::from(self.window))
            .min(self.count);
        let out = (self.next..end)
            .map(|i| {
                let start = i.saturating_mul(self.size);
                let chunk = self
                    .body
                    .get(start..start.saturating_add(self.size).min(self.body.len()))
                    .unwrap_or(&[]);
                let more = if i + 1 < self.count { 0x04 } else { 0 };
                let segment = Segment {
                    sequence: seq(i),
                    window: proposed,
                };
                let apdu = Apdu {
                    flags: 0x08 | more,
                    pdu: Pdu::ComplexAck {
                        invoke_id: invoke,
                        segment: Some(segment),
                        service: self.service,
                        body: chunk,
                    },
                    trailing: &[],
                };
                let mut bytes = Vec::new();
                apdu.encode(&mut bytes);
                Response::Reply(bytes)
            })
            .collect();
        self.next = end.max(self.next);
        out
    }
}

/// A segment's sequence number: its index modulo 256 (§20.1.5.3).
pub(super) fn seq(i: usize) -> u8 {
    u8::try_from(i & 0xFF).unwrap_or(0)
}
