// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Reassembling a segmented answer (§5.4.4.2, §5.4.4.3): acknowledging each
//! window, asking again for a lost segment, and bounding what a peer may
//! make this device hold.

use super::machine::Tsm;
use super::receive::{BUFFER_OVERFLOW, INVALID_APDU_IN_THIS_STATE, SEGMENTATION_NOT_SUPPORTED};
use super::step::{Abandoned, Reply};
use super::txn::{State, Txn};
use crate::apdu::Segment;
use core::time::Duration;

impl<P: Copy + Ord> Tsm<P> {
    /// Segment 0 of a segmented answer: acknowledge it alone, settling the
    /// window, and start reassembling.
    pub(crate) fn first(
        &mut self,
        txn: &mut Txn,
        key: (P, u8),
        s: Segment,
        body: &[u8],
        more: bool,
        now: Duration,
    ) -> Option<Result<Reply, Abandoned>> {
        if s.sequence != 0 {
            return Some(self.abort(key, INVALID_APDU_IN_THIS_STATE));
        }
        if !self.config.segmented_response {
            return Some(self.abort(key, SEGMENTATION_NOT_SUPPORTED));
        }
        if body.len() > self.config.max_reassembly {
            return Some(self.abort(key, BUFFER_OVERFLOW));
        }
        let window = s.window.clamp(1, 127).min(self.config.window);
        self.ack(key, 0, window, false);
        if !more {
            return Some(Ok(Reply::Complex(body.to_vec())));
        }
        txn.state = State::SegmentedConfirmation {
            last: 0,
            initial: 0,
            window,
            data: body.to_vec(),
        };
        txn.deadline = now.saturating_add(self.config.segment_timeout.saturating_mul(4));
        None
    }

    /// A later segment: append it if it is the next, acknowledge at the end
    /// of each window and at the last segment, ask again when one is lost.
    pub(crate) fn segment(
        &mut self,
        txn: &mut Txn,
        key: (P, u8),
        s: Segment,
        body: &[u8],
        more: bool,
        now: Duration,
    ) -> Option<Result<Reply, Abandoned>> {
        let max = self.config.max_reassembly;
        let State::SegmentedConfirmation {
            last,
            initial,
            window,
            data,
        } = &mut txn.state
        else {
            return None;
        };
        txn.deadline = now.saturating_add(self.config.segment_timeout.saturating_mul(4));
        if s.sequence != last.wrapping_add(1) {
            let (l, w) = (*last, *window);
            *initial = l;
            self.ack(key, l, w, true);
            return None;
        }
        if data.len().saturating_add(body.len()) > max {
            return Some(self.abort(key, BUFFER_OVERFLOW));
        }
        data.extend_from_slice(body);
        *last = s.sequence;
        if !more {
            let (w, all) = (*window, core::mem::take(data));
            self.ack(key, s.sequence, w, false);
            return Some(Ok(Reply::Complex(all)));
        }
        if s.sequence == initial.wrapping_add(*window) {
            *initial = s.sequence;
            let w = *window;
            self.ack(key, s.sequence, w, false);
        }
        None
    }
}
