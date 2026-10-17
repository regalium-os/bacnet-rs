// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The scheduler: run what is due — polls, renewals — and wait for
//! notifications in between.

use crate::client::Client;
use crate::error::Error;
use crate::watch::{Source, Update};
use regalium_bacnet_core::datalink::Datalink;
use regalium_bacnet_core::service::{PropertyReference, ReadAccess};
use regalium_bacnet_telemetry::Span;
use std::time::Duration;

impl<D: Datalink> Client<D> {
    /// Keeps every watch up to date for `wait`: polls what is due, renews
    /// subscriptions before they lapse, seeks devices that went quiet, and
    /// receives notifications, all of which arrive through
    /// [`Client::updates`]. Returns when `wait` has
    /// passed; call it again to go on.
    ///
    /// # Errors
    ///
    /// The datalink's. A device that stops answering is an event, and the
    /// poll that found it so is an update carrying the error.
    pub fn listen(&mut self, wait: Duration) -> Result<(), D::Error> {
        let deadline = self.now().saturating_add(wait);
        loop {
            self.attend()?;
            self.rebind_due()?;
            let now = self.now();
            if now >= deadline {
                return Ok(());
            }
            let next = self
                .watches
                .values()
                .map(|w| w.due)
                .chain(self.rebind_deadline())
                .min()
                .map_or(deadline, |d| d.min(deadline));
            self.pump(next, |_| false)?;
        }
    }

    /// Everything due now: a poll for each polled watch, a renewal for each
    /// subscription near its end. A renewal that fails turns the watch to
    /// polling, at once.
    fn attend(&mut self) -> Result<(), D::Error> {
        let now = self.now();
        let due: Vec<u32> = self
            .watches
            .iter()
            .filter(|(_, w)| w.due <= now)
            .map(|(id, _)| *id)
            .collect();
        if due.is_empty() {
            return Ok(());
        }
        let tracer = self.tracer.clone();
        let _span = Span::start(&*tracer, "bacnet.runtime.poll");
        for id in due {
            let Some(w) = self.watches.get(&id).cloned() else {
                continue;
            };
            if w.subscribed
                && let Some(lifetime) = w.watch.cov
            {
                let renewed = self.subscribe(id, &w.watch, lifetime)?;
                let after_now = self.now();
                if let Some(entry) = self.watches.get_mut(&id) {
                    if let Ok(after) = renewed {
                        entry.due = after_now.saturating_add(after);
                    } else {
                        entry.subscribed = false;
                        entry.due = now;
                    }
                }
                continue;
            }
            let reads = [ReadAccess {
                object: w.watch.object,
                properties: w
                    .watch
                    .properties
                    .iter()
                    .map(|p| PropertyReference::new(*p))
                    .collect(),
            }];
            let values = match self.read_multiple(w.watch.device, &reads) {
                Ok(values) => values,
                Err(Error::Request(e)) => vec![Err(e)],
                Err(Error::Link(e)) => return Err(e),
            };
            let polled = self.now();
            if let Some(entry) = self.watches.get_mut(&id) {
                entry.due = polled.saturating_add(w.watch.every);
            }
            let source = Source::Poll;
            self.update(Update {
                watch: id,
                device: w.watch.device,
                object: w.watch.object,
                source,
                values,
            });
        }
        Ok(())
    }
}
