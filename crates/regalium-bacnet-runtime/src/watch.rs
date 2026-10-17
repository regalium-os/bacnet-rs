// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Keeping points up to date: change of value where a device offers it,
//! polling where it does not, one stream of updates either way.

use crate::client::Client;
use crate::error::RequestError;
use crate::reading::Reading;
use regalium_bacnet_core::datalink::Datalink;
use regalium_bacnet_core::primitive::ObjectIdentifier;
use std::time::Duration;

/// What to keep up to date, and how.
///
/// `Watch::new` is present-value and status-flags, polled every 10 s and
/// pushed by a 300 s COV subscription where the device allows one. Every
/// field has a builder; adding one never breaks a caller.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Watch {
    pub(crate) device: u32,
    pub(crate) object: ObjectIdentifier,
    pub(crate) properties: Vec<u32>,
    pub(crate) every: Duration,
    pub(crate) cov: Option<Duration>,
}

impl Watch {
    /// Watches `object` of `device`.
    #[must_use]
    pub fn new(device: u32, object: ObjectIdentifier) -> Self {
        Self {
            device,
            object,
            properties: vec![85, 111],
            every: Duration::from_secs(10),
            cov: Some(Duration::from_secs(300)),
        }
    }

    /// The properties a poll reads. COV brings what the device chooses to
    /// send, which for most objects is present-value and status-flags.
    #[must_use]
    pub fn properties(mut self, properties: Vec<u32>) -> Self {
        self.properties = properties;
        self
    }

    /// How often to poll — always when polling, and from the moment a
    /// subscription is refused or lost.
    #[must_use]
    pub const fn every(mut self, every: Duration) -> Self {
        self.every = every;
        self
    }

    /// The COV subscription's lifetime, renewed before it lapses; `None`
    /// polls only. A lifetime is always asked for: an indefinite
    /// subscription is one a device keeps after this client is gone.
    #[must_use]
    pub const fn cov(mut self, lifetime: Option<Duration>) -> Self {
        self.cov = lifetime;
        self
    }
}

/// How an update was learnt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// The device pushed it: a COV notification.
    Cov,
    /// The client asked: a poll.
    Poll,
}

/// New values for a watched object.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Update {
    /// The watch, as [`Client::watch`] numbered it.
    pub watch: u32,
    /// The device.
    pub device: u32,
    /// The object.
    pub object: ObjectIdentifier,
    /// How it was learnt.
    pub source: Source,
    /// A value per property; a poll that failed whole carries its error.
    pub values: Vec<Result<Reading, RequestError>>,
}

/// How a watch is kept, and when it next needs attention.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Watched {
    pub(crate) watch: Watch,
    /// Subscribed, rather than polled.
    pub(crate) subscribed: bool,
    /// When the next poll or renewal is due.
    pub(crate) due: Duration,
}

impl<D: Datalink> Client<D> {
    /// Starts keeping `watch` up to date, by COV when it asks for it and the
    /// device agrees, by polling otherwise. Returns the watch's number,
    /// which its updates carry. Values arrive while [`Client::listen`] runs.
    ///
    /// # Errors
    ///
    /// The datalink's. A refused subscription is not an error: the watch
    /// polls instead, and its updates say so.
    pub fn watch(&mut self, watch: Watch) -> Result<u32, D::Error> {
        let id = self.next_watch;
        self.next_watch = self.next_watch.wrapping_add(1).max(1);
        let now = self.now();
        let mut watched = Watched {
            watch,
            subscribed: false,
            due: now,
        };
        if let Some(lifetime) = watched.watch.cov
            && let Ok(renew) = self.subscribe(id, &watched.watch, lifetime)?
        {
            watched.subscribed = true;
            watched.due = now.saturating_add(renew);
        }
        self.watches.insert(id, watched);
        Ok(id)
    }

    /// Stops a watch, cancelling its subscription when it has one.
    ///
    /// # Errors
    ///
    /// The datalink's. A cancellation the device refuses is ignored: the
    /// subscription lapses on its own.
    pub fn unwatch(&mut self, id: u32) -> Result<(), D::Error> {
        if let Some(w) = self.watches.remove(&id)
            && w.subscribed
        {
            let _ = self.cancel(id, &w.watch)?;
        }
        Ok(())
    }

    /// The updates since the last call, oldest first.
    pub fn updates(&mut self) -> impl Iterator<Item = Update> + '_ {
        self.updates.drain(..)
    }

    pub(crate) fn update(&mut self, u: Update) {
        while self.config.events > 0 && self.updates.len() >= self.config.events {
            self.updates.pop_front();
        }
        if self.config.events > 0 {
            self.updates.push_back(u);
        }
    }
}
