// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Discovery, as a trace and a log tell it: one span per sweep, recording
//! how many datagrams went out and how many answered, and a line per device
//! that did — which is how a reader finds out why a device is missing.

use super::{Client, Device, Discovery};
use crate::names;
use crate::service::who::{IHave, Wanted};
use regalium_bacnet_telemetry::{Attribute, Level, keys};
use std::fmt::Display;
use std::io;

impl Client {
    /// Runs a [`Discovery`] — Who-Is to every target, at the plan's rate —
    /// returning the devices that answered I-Am, sorted by instance.
    ///
    /// # Errors
    ///
    /// When the socket fails.
    pub fn discover(&mut self, plan: &Discovery) -> io::Result<Vec<Device>> {
        let span = self.enter("bacnet.client.discover");
        self.set(Attribute::int(keys::DATAGRAMS, count(plan.len())));
        let result = self.sweep_who_is(plan);
        if let Ok(found) = &result {
            self.set(Attribute::int(keys::FOUND, count(found.len())));
            for d in found {
                let route = d
                    .route
                    .as_ref()
                    .map_or_else(String::new, |r| format!(" via network {}", r.net));
                let attrs = [Attribute::int(keys::DEVICE_INSTANCE, d.instance.into())];
                self.log(
                    Level::Debug,
                    &format_args!(
                        "I-Am: device {} at {}{route}, vendor {}",
                        d.instance, d.addr, d.vendor
                    ),
                    &attrs,
                );
            }
            self.log(
                Level::Info,
                &format_args!(
                    "Who-Is: {} datagram(s), {} device(s) answered",
                    plan.len(),
                    found.len()
                ),
                &[],
            );
        }
        self.done(span, &result);
        result
    }

    /// Who-Has: which devices hold `wanted`.
    ///
    /// # Errors
    ///
    /// When the socket fails.
    pub fn who_has(
        &mut self,
        plan: &Discovery,
        wanted: &Wanted,
    ) -> io::Result<Vec<(Device, IHave)>> {
        let span = self.enter("bacnet.client.whoHas");
        self.set(Attribute::int(keys::DATAGRAMS, count(plan.len())));
        let result = self.sweep_who_has(plan, wanted);
        if let Ok(found) = &result {
            self.set(Attribute::int(keys::FOUND, count(found.len())));
            let what = match wanted {
                Wanted::Object(id) => names::object(*id),
                Wanted::Name(n) => format!("{n:?}"),
            };
            for (d, have) in found {
                self.log(
                    Level::Debug,
                    &format_args!(
                        "I-Have: device {} holds {} {:?}",
                        d.instance,
                        names::object(have.object),
                        have.name
                    ),
                    &[],
                );
            }
            self.log(
                Level::Info,
                &format_args!("Who-Has {what}: {} answer(s)", found.len()),
                &[],
            );
        }
        self.done(span, &result);
        result
    }

    /// Ends a sweep's span, failed with a warning if the socket failed.
    fn done<T>(&mut self, span: regalium_bacnet_telemetry::SpanId, result: &io::Result<T>) {
        if let Err(e) = result {
            self.log(Level::Warn, &format_args!("discovery failed: {e}"), &[]);
        }
        self.exit(span, result.as_ref().err().map(|e| e as &dyn Display));
    }
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}
