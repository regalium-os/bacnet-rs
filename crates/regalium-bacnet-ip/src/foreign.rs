// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Foreign device registration (Annex J.5.2), as a state machine on a clock
//! the caller supplies: when to send Register-Foreign-Device, what a
//! BVLC-Result means, and when the registration must be renewed.
//!
//! A host on a subnet without a BBMD — a laptop on the operator network, a
//! gateway in a cabinet — hears no broadcast from the lift controllers'
//! subnet. Registering with that subnet's BBMD makes it a member: the BBMD
//! sends it every broadcast, and distributes its broadcasts. The BBMD
//! forgets it 30 seconds after the time-to-live, so it renews first.

use crate::bvll::{Bvll, Function, result};
use core::time::Duration;
use std::net::SocketAddrV4;

/// How long after a refusal to ask again. A full table frees up when a
/// registration lapses; asking every retry interval would only add load.
const AFTER_REFUSAL: Duration = Duration::from_secs(30);

/// What a BVLC-Result did to the registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    /// Registered until `expires`; renewal is due well before.
    Registered {
        /// When the BBMD's entry, without renewal, stops counting this host.
        expires: Duration,
    },
    /// Refused, with this BVLC-Result code; asked again later.
    Refused(u16),
}

/// A registration with one BBMD.
///
/// Plain data: `Send` and `Sync`. Owned by the link that sends for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registration {
    bbmd: SocketAddrV4,
    ttl: u16,
    retry: Duration,
    waiting: bool,
    due: Duration,
    expires: Option<Duration>,
}

impl Registration {
    /// A registration with `bbmd` for `ttl` seconds, due at once. `Default`
    /// retry interval: 2 s, as the TSM's segment timeout.
    #[must_use]
    pub const fn new(bbmd: SocketAddrV4, ttl: u16) -> Self {
        Self {
            bbmd,
            ttl,
            retry: Duration::from_secs(2),
            waiting: false,
            due: Duration::ZERO,
            expires: None,
        }
    }

    /// How long to wait for a BVLC-Result before asking again.
    #[must_use]
    pub const fn retry_after(mut self, retry: Duration) -> Self {
        self.retry = retry;
        self
    }

    /// The BBMD.
    #[must_use]
    pub const fn bbmd(&self) -> SocketAddrV4 {
        self.bbmd
    }

    /// When [`Registration::poll`] next has something to send.
    #[must_use]
    pub const fn deadline(&self) -> Duration {
        self.due
    }

    /// Whether the BBMD counts this host as registered at `now`.
    #[must_use]
    pub fn is_registered(&self, now: Duration) -> bool {
        self.expires.is_some_and(|e| now < e)
    }

    /// The Register-Foreign-Device to send at `now`, when one is due: the
    /// first, a retry after silence, or a renewal.
    pub fn poll(&mut self, now: Duration) -> Option<Vec<u8>> {
        if now < self.due {
            return None;
        }
        self.waiting = true;
        self.due = now.saturating_add(self.retry);
        Some(Bvll::to_vec(Function::RegisterForeignDevice {
            ttl: self.ttl,
        }))
    }

    /// A BVLC-Result from `from` at `now`. `None` when it is not the answer
    /// this registration is waiting for — from another host, or unasked.
    pub fn result(&mut self, from: SocketAddrV4, code: u16, now: Duration) -> Option<Outcome> {
        if from != self.bbmd || !self.waiting {
            return None;
        }
        self.waiting = false;
        if code != result::SUCCESS {
            self.due = now.saturating_add(AFTER_REFUSAL);
            return Some(Outcome::Refused(code));
        }
        let ttl = Duration::from_secs(self.ttl.into());
        // Renew 30 s early when the TTL allows, half way when it is short:
        // a renewal that is itself lost still has time to be retried.
        let early = if self.ttl > 60 {
            ttl.saturating_sub(Duration::from_secs(30))
        } else {
            ttl / 2
        };
        self.due = now.saturating_add(early.max(Duration::from_secs(1)));
        let expires = now.saturating_add(ttl);
        self.expires = Some(expires);
        Some(Outcome::Registered { expires })
    }
}
