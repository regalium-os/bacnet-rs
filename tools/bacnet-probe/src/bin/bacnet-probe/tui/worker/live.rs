// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Following the selected device's points: COV subscriptions where the
//! device accepts them, polling for the rest, renewal before lapse, and
//! cancellation when the selection moves on.

use super::{Event, Job, Worker};
use bacnet_probe::metrics;
use bacnet_probe::{Device, ObjectId, property};
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// Lifetime asked for; renewed at three quarters of it.
const LIFETIME: u32 = 120;

#[derive(Default)]
pub(super) struct Live {
    device: Option<Device>,
    points: Vec<(ObjectId, Option<u32>)>,
    subscribed: bool,
    pub(super) interval: Duration,
    next_poll: Option<Instant>,
    renew_at: Option<Instant>,
    refused: HashSet<u32>,
    cancel: Vec<(Device, ObjectId, u32)>,
}

impl Live {
    /// Replaces what is followed; old subscriptions are cancelled next step.
    pub(super) fn want(&mut self, watch: Option<(Device, Vec<ObjectId>)>) {
        if let Some(device) = self.device.take() {
            self.cancel.extend(
                self.points
                    .drain(..)
                    .filter_map(|(id, p)| Some((device.clone(), id, p?))),
            );
        }
        if self.interval.is_zero() {
            self.interval = Duration::from_secs(2);
        }
        if let Some((device, ids)) = watch {
            self.points = ids.into_iter().map(|id| (id, None)).collect();
            self.device = Some(device);
            self.subscribed = false;
            self.next_poll = None;
        }
    }

    /// Forgets everything but the poll interval: what a device refused on
    /// one network says nothing about the device answering on the next.
    pub(super) fn reset(&mut self) {
        let interval = self.interval;
        *self = Self::default();
        self.interval = interval;
    }

    /// Takes the subscriptions still to be cancelled.
    pub(super) fn take_cancels(&mut self) -> Vec<(Device, ObjectId, u32)> {
        std::mem::take(&mut self.cancel)
    }

    pub(super) fn active(&self) -> bool {
        self.device.is_some() || !self.cancel.is_empty()
    }
}

impl Worker {
    /// One round of following: cancel, subscribe, listen, poll, renew —
    /// each stopping early when a command arrives.
    pub(super) fn live_step(&mut self) {
        while let Some((device, id, process)) = self.live.cancel.pop() {
            let _ = self.client.cancel_cov(&device, id, process);
            if self.superseded(Job::Live) {
                return;
            }
        }
        let Some(device) = self.live.device.clone() else {
            return;
        };
        if !self.live.subscribed {
            for i in 0..self.live.points.len() {
                if self.live.refused.contains(&device.instance) || self.superseded(Job::Live) {
                    break;
                }
                let Some(&(id, None)) = self.live.points.get(i) else {
                    continue;
                };
                match self.client.subscribe_cov(&device, id, LIFETIME) {
                    Ok(p) => {
                        if let Some(point) = self.live.points.get_mut(i) {
                            point.1 = Some(p);
                        }
                    }
                    Err(e) if e.unsupported() => {
                        self.live.refused.insert(device.instance);
                    }
                    Err(_) => {}
                }
            }
            self.live.subscribed = true;
            self.live.renew_at =
                Some(Instant::now() + Duration::from_secs(u64::from(LIFETIME) * 3 / 4));
            let cov = self.live.points.iter().filter(|(_, p)| p.is_some()).count();
            self.emit(Event::Following {
                cov,
                polled: self.live.points.len() - cov,
            });
        }
        let now = Instant::now();
        let poll_at = *self.live.next_poll.get_or_insert(now);
        let wait = poll_at
            .saturating_duration_since(now)
            .min(Duration::from_millis(100));
        if let Ok(notes) = self.client.notifications(wait) {
            for n in notes
                .into_iter()
                .filter(|n| n.device.instance == device.instance)
            {
                if let Some(v) = n
                    .values
                    .iter()
                    .find(|(p, _)| p.property == property::PRESENT_VALUE)
                    .and_then(|(_, v)| v.first())
                {
                    self.value(device.instance, n.object, Ok(v.clone()));
                }
            }
        }
        if Instant::now() >= poll_at {
            for (id, _) in self
                .live
                .points
                .clone()
                .into_iter()
                .filter(|(_, p)| p.is_none())
            {
                if self.superseded(Job::Live) {
                    return;
                }
                let value = self
                    .client
                    .read_property(&device, id, property::PRESENT_VALUE, None)
                    .map_err(|e| e.to_string())
                    .and_then(|v| v.into_iter().next().ok_or_else(|| "empty reply".to_owned()));
                self.value(device.instance, id, value);
            }
            self.live.next_poll = Some(Instant::now() + self.live.interval);
        }
        if self.live.renew_at.is_some_and(|t| Instant::now() >= t) {
            for (id, process) in self.live.points.clone() {
                if let Some(p) = process {
                    let _ = self.client.renew_cov(&device, id, p, LIFETIME);
                }
            }
            self.live.renew_at =
                Some(Instant::now() + Duration::from_secs(u64::from(LIFETIME) * 3 / 4));
        }
    }

    fn value(&mut self, instance: u32, id: ObjectId, value: Result<bacnet_probe::Value, String>) {
        let device = self.names.get(&instance).cloned().unwrap_or_default();
        let (name, units) = self
            .labels
            .get(&(instance, id))
            .cloned()
            .unwrap_or_default();
        let identity = self.client.identity_or(instance, &device);
        metrics::record_value(
            self.client.meter(),
            &identity,
            id,
            &name,
            units,
            value.as_ref().ok(),
        );
        self.emit(Event::Value(instance, id, value));
    }
}
