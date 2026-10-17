// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! How a client behaves: the transaction timers, and how many requests and
//! events it holds at once.

use regalium_bacnet_core::tsm;

/// A client's configuration.
///
/// `Default` is the transaction state machine's defaults (3 s, 3 retries),
/// 32 requests in flight and 1024 events held. Every field has a builder
/// method; adding one never breaks a caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub(crate) tsm: tsm::Config,
    pub(crate) in_flight: usize,
    pub(crate) events: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tsm: tsm::Config::default(),
            in_flight: 32,
            events: 1024,
        }
    }
}

impl Config {
    /// The transaction timers and limits.
    #[must_use]
    pub const fn transactions(mut self, tsm: tsm::Config) -> Self {
        self.tsm = tsm;
        self
    }

    /// How many requests a batch keeps outstanding at once. A device answers
    /// one request at a time on MS/TP; a batch that floods it only queues in
    /// the router, so the default is modest.
    #[must_use]
    pub const fn in_flight(mut self, n: usize) -> Self {
        self.in_flight = if n == 0 { 1 } else { n };
        self
    }

    /// How many events are kept for [`Client::events`](crate::Client::events)
    /// before the oldest are dropped. A caller that never drains them loses
    /// history, never memory.
    #[must_use]
    pub const fn events(mut self, n: usize) -> Self {
        self.events = n;
        self
    }
}
