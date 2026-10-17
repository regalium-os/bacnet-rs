// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What an arriving APDU does to the transaction it belongs to (§5.4.4).

use super::machine::Tsm;
use super::step::{Abandoned, Received, Reply, Step};
use super::txn::{State, Txn, abort, segment_ack, seq};
use crate::apdu::{Apdu, Pdu};
use core::time::Duration;

/// Abort reasons this machine sends (§20.1.9.3, `BACnetAbortReason`).
pub(crate) const BUFFER_OVERFLOW: u8 = 1;
pub(crate) const INVALID_APDU_IN_THIS_STATE: u8 = 2;
pub(crate) const SEGMENTATION_NOT_SUPPORTED: u8 = 4;

impl<P: Copy + Ord> Tsm<P> {
    /// Hands the machine an APDU that arrived from `peer` at `now`. Borrows
    /// it only for the call: whatever must outlive it is copied.
    ///
    /// A late or duplicated answer is [`Received::Unknown`], not an error:
    /// on a shared wire it is routine.
    pub fn receive(&mut self, peer: P, apdu: &Apdu<'_>, now: Duration) -> Received {
        let (invoke, service) = match apdu.pdu {
            Pdu::ConfirmedRequest { .. } | Pdu::UnconfirmedRequest { .. } => {
                return Received::NotForClient;
            }
            Pdu::SegmentAck { .. } | Pdu::Abort { .. } if !apdu.from_server() => {
                return Received::NotForClient;
            }
            Pdu::SimpleAck { invoke_id, service }
            | Pdu::ComplexAck {
                invoke_id, service, ..
            }
            | Pdu::Error {
                invoke_id, service, ..
            } => (invoke_id, Some(service)),
            Pdu::SegmentAck { invoke_id, .. }
            | Pdu::Reject { invoke_id, .. }
            | Pdu::Abort { invoke_id, .. } => (invoke_id, None),
        };
        let key = (peer, invoke);
        let Some(mut txn) = self.open.remove(&key) else {
            return Received::Unknown;
        };
        if service.is_some_and(|s| s != txn.service) {
            self.open.insert(key, txn);
            return Received::Unknown;
        }
        let reassembling = matches!(txn.state, State::SegmentedConfirmation { .. });
        let more = apdu.more_follows();
        let end = match apdu.pdu {
            Pdu::Reject { reason, .. } => Some(Ok(Reply::Reject(reason))),
            Pdu::Abort { reason, .. } => Some(Ok(Reply::Abort(reason))),
            Pdu::ComplexAck {
                segment: Some(s),
                body,
                ..
            } if reassembling => self.segment(&mut txn, key, s, body, more, now),
            _ if reassembling => Some(self.abort(key, INVALID_APDU_IN_THIS_STATE)),
            Pdu::SegmentAck {
                sequence, window, ..
            } => self.acked(&mut txn, key, sequence, window, apdu.is_negative(), now),
            Pdu::SimpleAck { .. } => Some(Ok(Reply::Simple)),
            Pdu::Error { body, .. } => Some(Ok(Reply::Error(body.to_vec()))),
            Pdu::ComplexAck {
                segment: None,
                body,
                ..
            } => Some(Ok(Reply::Complex(body.to_vec()))),
            Pdu::ComplexAck {
                segment: Some(s),
                body,
                ..
            } => self.first(&mut txn, key, s, body, more, now),
            Pdu::ConfirmedRequest { .. } | Pdu::UnconfirmedRequest { .. } => None,
        };
        match end {
            Some(end) => self.finish(&txn, key, end),
            None => {
                self.open.insert(key, txn);
            }
        }
        Received::Matched
    }

    /// A Segment-ACK for a segmented request: slide the window past the
    /// segment acknowledged, or — all acknowledged — wait for the answer.
    fn acked(
        &mut self,
        txn: &mut Txn,
        key: (P, u8),
        sequence: u8,
        window: u8,
        nak: bool,
        now: Duration,
    ) -> Option<Result<Reply, Abandoned>> {
        let State::SegmentedRequest { base, next, .. } = txn.state else {
            return Some(self.abort(key, INVALID_APDU_IN_THIS_STATE));
        };
        // A window is at most 127, so the sequence number names one segment
        // in flight unambiguously. A negative acknowledgement may also name
        // the segment before the window — everything in it was lost — and
        // asks for the window again; a positive one naming it is a duplicate.
        let from = if nak { base.saturating_sub(1) } else { base };
        let Some(acked) = (from..next).find(|i| seq(*i) == sequence) else {
            txn.deadline = now.saturating_add(self.config.segment_timeout);
            return None;
        };
        let base = acked + 1;
        txn.window_retries = 0;
        if base >= txn.count {
            txn.state = State::AwaitConfirmation;
            txn.deadline = now.saturating_add(self.config.apdu_timeout);
            return None;
        }
        // A negative acknowledgement and a positive one both mean "received
        // through `sequence`": send on from the segment after it.
        txn.state = State::SegmentedRequest {
            base,
            next: base,
            window: window.clamp(1, 127),
        };
        txn.deadline = now.saturating_add(self.config.segment_timeout);
        txn.fill(key.0, key.1, &self.config, &mut self.steps);
        None
    }

    pub(crate) fn ack(&mut self, (peer, invoke): (P, u8), sequence: u8, window: u8, nak: bool) {
        let apdu = segment_ack(invoke, sequence, window, nak);
        self.steps.push_back(Step::Transmit {
            peer,
            invoke,
            attempt: 1,
            apdu,
        });
    }

    /// Sends an Abort and ends the transaction with it.
    pub(crate) fn abort(
        &mut self,
        (peer, invoke): (P, u8),
        reason: u8,
    ) -> Result<Reply, Abandoned> {
        let apdu = abort(invoke, reason);
        self.steps.push_back(Step::Transmit {
            peer,
            invoke,
            attempt: 1,
            apdu,
        });
        Err(Abandoned::Aborted(reason))
    }
}
