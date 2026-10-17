// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The client state machine: opening a transaction, its timers, and how it
//! ends. Opening one is in `open.rs`, receiving in `receive.rs`.

use super::config::Config;
use super::step::{Abandoned, Reply, Step};
use super::txn::{State, Txn};
use core::time::Duration;
use regalium_bacnet_telemetry::{Attribute, NoopTracer, Tracer, keys};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

/// A client's transaction state machine (§5.4.4), for every peer at once.
///
/// It is handed requests, received APDUs and the time, and answers with
/// [`Step`]s; it never sends, sleeps or reads a clock. `P` names a peer —
/// `Copy + Ord` so transactions are kept in a deterministic order, which is
/// what makes a scenario replay identically.
///
/// # Concurrency
///
/// `Send` when `P` is. Every method takes `&mut self`: one task owns the
/// machine and feeds it, which is the only correct use — two tasks racing
/// to `poll` would split one transaction's steps between them.
pub struct Tsm<P> {
    pub(crate) config: Config,
    pub(crate) open: BTreeMap<(P, u8), Txn>,
    pub(crate) steps: VecDeque<Step<P>>,
    pub(crate) next_invoke: u8,
    pub(crate) tracer: Arc<dyn Tracer>,
}

impl<P: Copy + Ord> Tsm<P> {
    /// A machine with no open transactions. Spawns nothing, reads nothing.
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            open: BTreeMap::new(),
            steps: VecDeque::new(),
            next_invoke: 0,
            tracer: Arc::new(NoopTracer),
        }
    }

    /// Opens a `bacnet.tsm.transaction` span per transaction through
    /// `tracer`, recording invoke id, service, attempts and outcome.
    #[must_use]
    pub fn with_tracer(mut self, tracer: Arc<dyn Tracer>) -> Self {
        self.tracer = tracer;
        self
    }

    /// How many transactions are open.
    #[must_use]
    pub fn open(&self) -> usize {
        self.open.len()
    }

    /// The next thing to do at `now`: something queued, else a timeout
    /// that has fallen due, else [`Step::Idle`] until the next deadline.
    /// Call it until it returns `Idle`, after every request and receive
    /// and whenever the deadline passes.
    pub fn poll(&mut self, now: Duration) -> Step<P> {
        if self.steps.is_empty() {
            let due: Vec<_> = self
                .open
                .iter()
                .filter(|(_, t)| t.deadline <= now)
                .map(|(k, _)| *k)
                .collect();
            for key in due {
                self.expire(key, now);
            }
        }
        self.steps.pop_front().unwrap_or_else(|| Step::Idle {
            wake_at: self.open.values().map(|t| t.deadline).min(),
        })
    }

    /// Sends the whole request from its first octet, unsegmented or as
    /// the first window of segments.
    pub(crate) fn send(&mut self, txn: &mut Txn, peer: P, invoke: u8, now: Duration) {
        if txn.count <= 1 {
            txn.state = State::AwaitConfirmation;
            txn.deadline = now.saturating_add(self.config.apdu_timeout);
            self.steps.push_back(Step::Transmit {
                peer,
                invoke,
                attempt: txn.attempts,
                apdu: txn.request_apdu(invoke, 0, &self.config),
            });
        } else {
            // The first segment goes alone: the server's Segment-ACK says
            // how large a window it takes (§5.4.4.1).
            txn.state = State::SegmentedRequest {
                base: 0,
                next: 0,
                window: 1,
            };
            txn.window_retries = 0;
            txn.deadline = now.saturating_add(self.config.segment_timeout);
            txn.fill(peer, invoke, &self.config, &mut self.steps);
        }
    }

    fn expire(&mut self, key: (P, u8), now: Duration) {
        let Some(mut txn) = self.open.remove(&key) else {
            return;
        };
        let (peer, invoke) = key;
        match txn.state {
            State::SegmentedConfirmation { .. } => {
                return self.finish(&txn, key, Err(Abandoned::SegmentTimeout));
            }
            State::SegmentedRequest { base, window, .. }
                if txn.window_retries < self.config.retries =>
            {
                txn.window_retries += 1;
                txn.state = State::SegmentedRequest {
                    base,
                    next: base,
                    window,
                };
                txn.deadline = now.saturating_add(self.config.segment_timeout);
                txn.fill(peer, invoke, &self.config, &mut self.steps);
            }
            State::AwaitConfirmation if txn.attempts <= self.config.retries => {
                txn.attempts += 1;
                self.send(&mut txn, peer, invoke, now);
            }
            _ => {
                let attempts = txn.attempts;
                return self.finish(&txn, key, Err(Abandoned::Timeout { attempts }));
            }
        }
        self.open.insert(key, txn);
    }

    /// Ends a transaction already removed from `open`: its span, and the
    /// step that tells the caller.
    pub(crate) fn finish(&mut self, txn: &Txn, key: (P, u8), end: Result<Reply, Abandoned>) {
        let (peer, invoke) = key;
        let outcome = match &end {
            Ok(Reply::Simple | Reply::Complex(_)) => "ok",
            Ok(Reply::Error(_)) => "error",
            Ok(Reply::Reject(_)) => "reject",
            Ok(Reply::Abort(_)) | Err(Abandoned::Aborted(_)) => "abort",
            Err(_) => "timeout",
        };
        self.tracer.attribute(
            txn.span,
            Attribute::int(keys::ATTEMPTS, txn.attempts.into()),
        );
        self.tracer
            .attribute(txn.span, Attribute::str(keys::OUTCOME, outcome));
        match end {
            Ok(reply) => {
                self.tracer.end(txn.span, None);
                self.steps.push_back(Step::Complete {
                    peer,
                    invoke,
                    reply,
                });
            }
            Err(reason) => {
                self.tracer.end(txn.span, Some(&reason));
                self.steps.push_back(Step::Abandon {
                    peer,
                    invoke,
                    reason,
                });
            }
        }
    }
}
