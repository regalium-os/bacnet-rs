// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The live loop: subscribe to COV where the device accepts it, poll the
//! rest, renew subscriptions before they lapse, and report every change.
//! `watch` prints the changes; `export` only records them as metrics.

use super::report::report;
use crate::Result;
use bacnet_probe::scan::ObjectReport;
use bacnet_probe::{Client, Device, ObjectId, Value, property};
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// A point being followed, with the last value seen (`None` while unknown
/// or unreachable).
pub(crate) struct Point {
    pub(crate) device: Device,
    pub(crate) device_name: String,
    pub(crate) object: ObjectId,
    pub(crate) name: String,
    pub(crate) units: Option<u32>,
    pub(crate) labels: Vec<Value>,
    pub(crate) last: Option<Value>,
    pub(crate) process: Option<u32>,
}

impl Point {
    /// A point from what a scan read. `fresh` forgets the scanned value, so
    /// the first reading prints as a change.
    pub(crate) fn new(device: &Device, device_name: &str, o: &ObjectReport, fresh: bool) -> Self {
        Self {
            device: device.clone(),
            device_name: device_name.to_owned(),
            object: o.id,
            name: o.name(),
            units: o.units(),
            labels: o.labels(),
            last: if fresh {
                None
            } else {
                o.present_value().cloned()
            },
            process: None,
        }
    }
}

/// Where changes go.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sink {
    Table,
    Json,
    /// Metrics only.
    Quiet,
}

/// How to follow.
pub(crate) struct Follow {
    /// Poll interval for points without a subscription.
    pub(crate) every: Duration,
    /// Try COV first.
    pub(crate) cov: bool,
    /// Subscription lifetime, seconds; renewed at three quarters.
    pub(crate) lifetime: u32,
    pub(crate) sink: Sink,
    /// Stop then (for `export`'s periodic rescans); `None` runs forever.
    pub(crate) until: Option<Instant>,
}

pub(crate) fn follow(client: &mut Client, points: &mut [Point], how: &Follow) -> Result {
    let started = Instant::now();
    let mut refused: HashSet<u32> = HashSet::new();
    if how.cov {
        subscribe(client, points, how.lifetime, &mut refused);
    }
    let by_cov = points.iter().filter(|p| p.process.is_some()).count();
    eprintln!(
        "Following {} points: {by_cov} by COV, {} polled every {:?}. Ctrl-C to stop.",
        points.len(),
        points.len() - by_cov,
        how.every
    );
    let renew_every = Duration::from_secs(u64::from(how.lifetime) * 3 / 4);
    let (mut next_poll, mut renew_at) = (Instant::now(), Instant::now() + renew_every);
    loop {
        let now = Instant::now();
        if how.until.is_some_and(|u| now >= u) {
            return Ok(());
        }
        for n in client.notifications(
            next_poll
                .saturating_duration_since(now)
                .min(Duration::from_millis(500)),
        )? {
            let pv = n
                .values
                .iter()
                .find(|(p, _)| p.property == property::PRESENT_VALUE)
                .and_then(|(_, v)| v.first());
            let hit = points.iter_mut().find(|p| {
                p.process == Some(n.process)
                    && p.object == n.object
                    && p.device.instance == n.device.instance
            });
            if let (Some(p), Some(v)) = (hit, pv) {
                report(client, p, Ok(v.clone()), how.sink, started);
            }
        }
        if Instant::now() >= next_poll {
            for p in points.iter_mut().filter(|p| p.process.is_none()) {
                let now = client
                    .read_property(&p.device, p.object, property::PRESENT_VALUE, None)
                    .map_err(|e| e.to_string())
                    .and_then(|v| v.into_iter().next().ok_or_else(|| "empty reply".to_owned()));
                report(client, p, now, how.sink, started);
            }
            next_poll = Instant::now() + how.every;
        }
        if Instant::now() >= renew_at {
            for p in points.iter_mut() {
                if let Some(process) = p.process
                    && client
                        .renew_cov(&p.device, p.object, process, how.lifetime)
                        .is_err()
                {
                    p.process = None; // Poll it until the next rescan.
                }
            }
            renew_at = Instant::now() + renew_every;
        }
    }
}

fn subscribe(client: &mut Client, points: &mut [Point], lifetime: u32, refused: &mut HashSet<u32>) {
    for p in points.iter_mut() {
        if refused.contains(&p.device.instance) {
            continue;
        }
        match client.subscribe_cov(&p.device, p.object, lifetime) {
            Ok(process) => p.process = Some(process),
            Err(e) if e.unsupported() => {
                refused.insert(p.device.instance);
            }
            Err(_) => {} // This object will not; poll it.
        }
    }
}
