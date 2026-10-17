// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Discovery (§16.10): Who-Is out, I-Ams in, and each answer bound.

use crate::client::Client;
use crate::device::Device;
use regalium_bacnet_core::datalink::{Datalink, Recipient};
use regalium_bacnet_core::npdu::{Address, Body, Destination, GLOBAL, Npdu};
use regalium_bacnet_core::service::{WhoIs, unconfirmed};
use regalium_bacnet_telemetry::{Attribute, Span, keys};
use std::time::Duration;

impl<D: Datalink> Client<D> {
    /// Broadcasts Who-Is to every network and binds every device that
    /// answers within `wait`, behind routers included. `range` narrows it to
    /// instances `low..=high`, which on a large site is the polite way to
    /// ask. Returns the devices that answered, by instance.
    ///
    /// # Errors
    ///
    /// The datalink's. Silence is an empty list.
    pub fn who_is(
        &mut self,
        range: Option<(u32, u32)>,
        wait: Duration,
    ) -> Result<Vec<Device<D::Mac>>, D::Error> {
        self.ask(Recipient::LocalBroadcast, range, wait, None)
    }

    /// Who-Is sent to one address — a device, or a router whose broadcasts
    /// do not reach here — for a network where broadcast is not an option.
    ///
    /// # Errors
    ///
    /// The datalink's.
    pub fn who_is_at(
        &mut self,
        to: D::Mac,
        range: Option<(u32, u32)>,
        wait: Duration,
    ) -> Result<Vec<Device<D::Mac>>, D::Error> {
        self.ask(Recipient::Unicast(to), range, wait, None)
    }

    /// Finds device `instance` by a Who-Is for it alone, returning as soon
    /// as it answers, or `None` after `wait`.
    ///
    /// # Errors
    ///
    /// The datalink's.
    pub fn bind_device(
        &mut self,
        instance: u32,
        wait: Duration,
    ) -> Result<Option<Device<D::Mac>>, D::Error> {
        let found = self.ask(
            Recipient::LocalBroadcast,
            Some((instance, instance)),
            wait,
            Some(instance),
        )?;
        Ok(found.into_iter().find(|d| d.instance == instance))
    }

    fn ask(
        &mut self,
        to: Recipient<D::Mac>,
        range: Option<(u32, u32)>,
        wait: Duration,
        until: Option<u32>,
    ) -> Result<Vec<Device<D::Mac>>, D::Error> {
        // The span outlives calls that borrow the client mutably.
        let tracer = self.tracer.clone();
        let span = Span::start(&*tracer, "bacnet.runtime.discover");
        let mut apdu = vec![0x10, unconfirmed::WHO_IS];
        WhoIs { range }.encode(&mut apdu);
        let npdu = Npdu {
            control: 0,
            destination: Some(Destination {
                address: Address {
                    network: GLOBAL,
                    mac: &[],
                },
                hop_count: 255,
            }),
            source: None,
            body: Body::Apdu(&apdu),
        };
        let mut out = Vec::new();
        npdu.encode(&mut out);
        self.heard.clear();
        if let Err(e) = self.link.send(to, &out) {
            span.fail(&e);
            return Err(e);
        }
        let deadline = self.now().saturating_add(wait);
        self.pump(deadline, |c| until.is_some_and(|i| c.heard.contains(&i)))?;
        let found: Vec<_> = self
            .heard
            .iter()
            .filter_map(|i| self.devices.get(i).cloned())
            .collect();
        span.set(Attribute::int(
            keys::FOUND,
            i64::try_from(found.len()).unwrap_or(i64::MAX),
        ));
        Ok(found)
    }
}
