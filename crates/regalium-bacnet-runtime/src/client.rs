// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The client: one datalink, the transaction state machine, and what it has
//! learnt about the devices on the network.

use crate::config::Config;
use crate::device::{Device, Event, Route};
use regalium_bacnet_core::datalink::Datalink;
use regalium_bacnet_core::tsm::{Abandoned, Reply, Tsm};
use regalium_bacnet_telemetry::{NoopTracer, Tracer};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A `BACnet` client over one datalink.
///
/// It owns the clock: the core is handed the time, this is what reads it.
/// [`Client::new`] does no I/O and starts nothing, so a client can be built,
/// inspected and dropped; every request blocks until it is answered or
/// abandoned, and dropping the client closes nothing it has not opened.
///
/// # Concurrency
///
/// `Send` when the datalink is. Every request takes `&mut self`: one thread
/// drives a client. For many requests at once, use
/// [`Client::read_many`], which keeps several in flight on that thread.
pub struct Client<D: Datalink> {
    pub(crate) link: D,
    pub(crate) config: Config,
    pub(crate) tsm: Tsm<u32>,
    pub(crate) devices: BTreeMap<u32, Device<D::Mac>>,
    /// Devices whose last request was abandoned.
    pub(crate) offline: BTreeSet<u32>,
    /// Devices heard from since the current discovery began.
    pub(crate) heard: BTreeSet<u32>,
    pub(crate) events: VecDeque<Event>,
    /// Finished transactions, by device and invoke id, until collected.
    pub(crate) done: BTreeMap<(u32, u8), Result<Reply, Abandoned>>,
    origin: Instant,
    pub(crate) tracer: Arc<dyn Tracer>,
}

impl<D: Datalink> Client<D> {
    /// A client on `link`. Sends nothing.
    #[must_use]
    pub fn new(link: D, config: Config) -> Self {
        Self {
            link,
            config,
            tsm: Tsm::new(config.tsm),
            devices: BTreeMap::new(),
            offline: BTreeSet::new(),
            heard: BTreeSet::new(),
            events: VecDeque::new(),
            done: BTreeMap::new(),
            origin: Instant::now(),
            tracer: Arc::new(NoopTracer),
        }
    }

    /// Opens `bacnet.runtime.*` spans through `tracer`, with each request's
    /// `bacnet.tsm.transaction` beneath.
    #[must_use]
    pub fn with_tracer(mut self, tracer: Arc<dyn Tracer>) -> Self {
        self.tsm = Tsm::new(self.config.tsm).with_tracer(tracer.clone());
        self.tracer = tracer;
        self
    }

    /// The datalink.
    #[must_use]
    pub const fn link(&self) -> &D {
        &self.link
    }

    /// Every device bound so far, by instance.
    pub fn devices(&self) -> impl Iterator<Item = &Device<D::Mac>> {
        self.devices.values()
    }

    /// One bound device.
    #[must_use]
    pub fn device(&self, instance: u32) -> Option<&Device<D::Mac>> {
        self.devices.get(&instance)
    }

    /// Binds a device without asking the network: one whose address and
    /// limits are known from configuration.
    pub fn add_device(&mut self, device: Device<D::Mac>) {
        self.devices.insert(device.instance, device);
    }

    /// The events since the last call, oldest first. Taking them empties the
    /// buffer; past [`Config::events`] the oldest were dropped.
    pub fn events(&mut self) -> impl Iterator<Item = Event> + '_ {
        self.events.drain(..)
    }

    /// The client's clock: time since it was built.
    pub(crate) fn now(&self) -> Duration {
        self.origin.elapsed()
    }

    pub(crate) fn event(&mut self, e: Event) {
        if self.config.events == 0 {
            return;
        }
        while self.events.len() >= self.config.events {
            self.events.pop_front();
        }
        self.events.push_back(e);
    }

    /// The device a datagram from `from` through `route` came from.
    pub(crate) fn find(&self, from: &D::Mac, route: Option<&Route>) -> Option<u32> {
        self.devices
            .values()
            .find(|d| d.address == *from && d.route.as_ref() == route)
            .map(|d| d.instance)
    }

    /// A device answered: if it was offline, it is back.
    pub(crate) fn reached(&mut self, instance: u32) {
        if self.offline.remove(&instance) {
            self.event(Event::Reachable(instance));
        }
    }

    /// A request to a device was abandoned: report it once, not per request.
    pub(crate) fn lost(&mut self, instance: u32, reason: Abandoned) {
        if self.offline.insert(instance) {
            self.event(Event::Offline { instance, reason });
        }
    }
}
