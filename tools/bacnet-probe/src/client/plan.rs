// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A discovery plan: where Who-Is (or Who-Has) goes, how fast, and whose
//! answers count.

use crate::frame::Delivery;
use crate::scope::{MAX_HOSTS, Scope};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

/// Where to ask, how fast, and whose answers to keep.
#[derive(Clone, Debug)]
pub struct Discovery {
    pub(super) targets: Vec<(SocketAddr, Delivery)>,
    pub(super) wait: Duration,
    pub(super) rate: u32,
    pub(super) range: Option<(u32, u32)>,
    allow: Vec<Scope>,
}

impl Discovery {
    /// One datagram to `target`. Waits 3 s for answers; no rate limit
    /// matters for one datagram.
    #[must_use]
    pub fn to(target: SocketAddr, delivery: Delivery) -> Self {
        Self {
            targets: vec![(target, delivery)],
            wait: Duration::from_secs(3),
            rate: 500,
            range: None,
            allow: Vec::new(),
        }
    }

    /// Every address in `scopes` on `port`: one broadcast per /24, or with
    /// `unicast` one datagram per host. Only answers from inside `scopes`
    /// are kept.
    ///
    /// # Errors
    ///
    /// When a unicast sweep would exceed [`MAX_HOSTS`].
    pub fn sweep(scopes: &[Scope], port: u16, unicast: bool) -> Result<Self, String> {
        let mut targets = Vec::new();
        for s in scopes {
            if unicast {
                if s.host_count() > MAX_HOSTS {
                    return Err(format!(
                        "{s} is {} hosts; a unicast sweep is limited to {MAX_HOSTS}",
                        s.host_count()
                    ));
                }
                targets.extend(
                    s.hosts()
                        .map(|ip| (SocketAddr::from((ip, port)), Delivery::Unicast)),
                );
            } else {
                let delivery = |b: bool| {
                    if b {
                        Delivery::Broadcast
                    } else {
                        Delivery::Unicast
                    }
                };
                targets.extend(
                    s.broadcasts()
                        .into_iter()
                        .map(|(ip, b)| (SocketAddr::from((ip, port)), delivery(b))),
                );
            }
        }
        let mut plan = Self::to(SocketAddr::from(([0, 0, 0, 0], port)), Delivery::Unicast)
            .allowing(scopes.to_vec());
        plan.targets = targets;
        Ok(plan)
    }

    /// How long to keep listening after the last datagram is sent.
    #[must_use]
    pub fn with_wait(mut self, wait: Duration) -> Self {
        self.wait = wait;
        self
    }

    /// Datagrams per second while sweeping. Defaults to 500: gentle enough
    /// for a site network shared with life-safety equipment.
    #[must_use]
    pub fn with_rate(mut self, per_second: u32) -> Self {
        self.rate = per_second.max(1);
        self
    }

    /// Only devices with instances in `low..=high` (§16.10.1).
    #[must_use]
    pub fn with_range(mut self, range: Option<(u32, u32)>) -> Self {
        self.range = range;
        self
    }

    /// Keeps only answers from inside `scopes`; empty keeps everything.
    #[must_use]
    pub fn allowing(mut self, scopes: Vec<Scope>) -> Self {
        self.allow = scopes;
        self
    }

    /// How many datagrams the sweep sends.
    #[must_use]
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether it sends nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    pub(super) fn allows(&self, from: SocketAddr) -> bool {
        match from.ip() {
            IpAddr::V4(ip) => self.allow.is_empty() || self.allow.iter().any(|s| s.contains(ip)),
            IpAddr::V6(_) => false,
        }
    }
}
