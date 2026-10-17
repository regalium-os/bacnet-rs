// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Opening a transaction: sizing the request against what the peer takes,
//! and choosing its invoke id.

use super::config::Request;
use super::machine::Tsm;
use super::step::Refused;
use super::txn::{State, Txn};
use crate::apdu::Segments;
use core::time::Duration;
use regalium_bacnet_telemetry::{Attribute, keys};

/// APDU header octets of an unsegmented and a segmented Confirmed-Request.
const HEADER: usize = 4;
const SEGMENTED_HEADER: usize = 6;

impl<P: Copy + Ord> Tsm<P> {
    /// Starts a transaction to `peer` at `now` and returns its invoke id.
    /// What to send is the next [`Tsm::poll`].
    ///
    /// The invoke id is unique among every request this machine has open,
    /// to any peer, so a reply can never be paired with the wrong request
    /// even when two peers share a router.
    ///
    /// # Errors
    ///
    /// [`Refused`] when no invoke id is free or the peer cannot take the
    /// request; nothing is sent and no transaction is opened.
    pub fn request(&mut self, peer: P, request: Request, now: Duration) -> Result<u8, Refused> {
        let remote = request.remote;
        let max = usize::from(remote.max_apdu);
        let len = request.body.len();
        let (size, count) = if HEADER.saturating_add(len) <= max {
            (len, 1)
        } else {
            if !remote.segmented_requests {
                return Err(Refused::NeedsSegmentation);
            }
            let size = max.saturating_sub(SEGMENTED_HEADER);
            if size == 0 {
                return Err(Refused::PeerTooSmall);
            }
            (size, len.div_ceil(size))
        };
        if let Segments::AtMost(n) = remote.max_segments
            && count > 1
            && count > usize::from(n)
        {
            return Err(Refused::TooManySegments { needed: count });
        }
        let invoke = self.allocate().ok_or(Refused::NoInvokeId)?;
        let span = self.tracer.start("bacnet.tsm.transaction", request.parent);
        self.tracer
            .attribute(span, Attribute::int(keys::INVOKE_ID, invoke.into()));
        self.tracer.attribute(
            span,
            Attribute::int(keys::SERVICE_CHOICE, request.service.into()),
        );
        self.tracer
            .attribute(span, Attribute::bool(keys::SEGMENTED, count > 1));
        let mut txn = Txn {
            service: request.service,
            body: request.body,
            size,
            count,
            state: State::AwaitConfirmation,
            deadline: now,
            attempts: 1,
            window_retries: 0,
            span,
        };
        self.send(&mut txn, peer, invoke, now);
        self.open.insert((peer, invoke), txn);
        Ok(invoke)
    }

    /// The next invoke id no open transaction holds, counting on from the
    /// last so a just-freed id is the last to be reused: a late answer to
    /// an abandoned request then finds nothing rather than its successor.
    fn allocate(&mut self) -> Option<u8> {
        (0..=u8::MAX)
            .map(|i| self.next_invoke.wrapping_add(i))
            .find(|id| !self.open.keys().any(|(_, open)| open == id))
            .inspect(|id| self.next_invoke = id.wrapping_add(1))
    }
}
