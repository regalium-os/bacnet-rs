// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Who-Is and Who-Has, to one address or swept over a range.

use super::{Client, Device, Discovery};
use crate::frame::{self, Delivery, GLOBAL_BROADCAST, Incoming};
use crate::service::who::{self, IHave, Wanted};
use crate::service::{self, unconfirmed};
use std::io;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

impl Client {
    /// Who-Is to `target`, collecting I-Am for `wait`.
    ///
    /// # Errors
    ///
    /// When the socket fails.
    pub fn who_is(
        &mut self,
        target: SocketAddr,
        broadcast: bool,
        wait: Duration,
    ) -> io::Result<Vec<Device>> {
        self.who_is_range(target, broadcast, wait, None)
    }

    /// Who-Is limited to instances `low..=high`; returns as soon as the one
    /// device asked for answers.
    ///
    /// # Errors
    ///
    /// When the socket fails.
    pub fn who_is_range(
        &mut self,
        target: SocketAddr,
        broadcast: bool,
        wait: Duration,
        range: Option<(u32, u32)>,
    ) -> io::Result<Vec<Device>> {
        let delivery = if broadcast {
            Delivery::Broadcast
        } else {
            Delivery::Unicast
        };
        self.discover(
            &Discovery::to(target, delivery)
                .with_wait(wait)
                .with_range(range),
        )
    }

    /// Runs a [`Discovery`], returning devices sorted by instance.
    ///
    /// # Errors
    ///
    /// When the socket fails.
    pub fn discover(&mut self, plan: &Discovery) -> io::Result<Vec<Device>> {
        let apdu = service::unconfirmed_request(unconfirmed::WHO_IS, &who::who_is(plan.range));
        let single = plan.range.is_some_and(|(l, h)| l == h);
        let mut found = self.sweep(plan, &apdu, single, |msg| {
            let i_am = msg
                .apdu
                .strip_prefix(&[0x10, unconfirmed::I_AM])
                .and_then(who::decode_i_am)?;
            let device = Device {
                instance: i_am.device.instance,
                addr: msg.from,
                route: msg.source.clone(),
                max_apdu: i_am.max_apdu,
                vendor: i_am.vendor,
            };
            plan.range
                .is_none_or(|(l, h)| (l..=h).contains(&device.instance))
                .then_some((device.instance, device))
        })?;
        found.sort_by_key(|d| d.instance);
        Ok(found)
    }

    /// Who-Has: which devices hold `wanted`. A device found this way did not
    /// say how big an APDU it takes, so it is assumed to take 480 octets.
    ///
    /// # Errors
    ///
    /// When the socket fails.
    pub fn who_has(
        &mut self,
        plan: &Discovery,
        wanted: &Wanted,
    ) -> io::Result<Vec<(Device, IHave)>> {
        let apdu =
            service::unconfirmed_request(unconfirmed::WHO_HAS, &who::who_has(plan.range, wanted));
        self.sweep(plan, &apdu, false, |msg| {
            let have = msg
                .apdu
                .strip_prefix(&[0x10, unconfirmed::I_HAVE])
                .and_then(who::decode_i_have)?;
            let device = Device {
                instance: have.device.instance,
                addr: msg.from,
                route: msg.source.clone(),
                max_apdu: 480,
                vendor: 0,
            };
            Some((have.device.instance, (device, have)))
        })
    }

    /// Sends `apdu` to every target at the plan's rate, collecting what
    /// `accept` keeps, deduplicated by its key.
    fn sweep<T>(
        &mut self,
        plan: &Discovery,
        apdu: &[u8],
        single: bool,
        mut accept: impl FnMut(&Incoming<'_>) -> Option<(u32, T)>,
    ) -> io::Result<Vec<T>> {
        let gap = Duration::from_secs_f64(1.0 / f64::from(plan.rate));
        let mut found: Vec<(u32, T)> = Vec::new();
        let mut distributed = false;
        let mut collect =
            |client: &mut Self, deadline: Instant, found: &mut Vec<(u32, T)>| -> io::Result<()> {
                while let Some((datagram, peer)) = client.recv_until(deadline)? {
                    let Some(msg) = frame::decode(&datagram, peer) else {
                        continue;
                    };
                    if client.unsolicited(&msg)? || !plan.allows(msg.from) {
                        continue;
                    }
                    if let Some((key, item)) = accept(&msg)
                        && !found.iter().any(|(k, _)| *k == key)
                    {
                        found.push((key, item));
                        if single {
                            return Ok(());
                        }
                    }
                }
                Ok(())
            };
        let _ = self.maintain();
        for &(target, delivery) in &plan.targets {
            let (to, how) = if delivery == Delivery::Broadcast {
                self.broadcast_via(target)
            } else {
                (target, delivery)
            };
            // Registered: one distribute covers every broadcast in the plan.
            if how == Delivery::Distribute && std::mem::replace(&mut distributed, true) {
                continue;
            }
            self.send(
                &frame::encode(how, Some(&GLOBAL_BROADCAST), false, apdu),
                to,
            )?;
            collect(self, Instant::now() + gap, &mut found)?;
            if single && !found.is_empty() {
                return Ok(found.into_iter().map(|(_, t)| t).collect());
            }
        }
        collect(self, Instant::now() + plan.wait, &mut found)?;
        Ok(found.into_iter().map(|(_, t)| t).collect())
    }
}
