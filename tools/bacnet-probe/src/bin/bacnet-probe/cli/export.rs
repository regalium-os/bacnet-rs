// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `export`: run headless and keep every point of every device flowing into
//! metrics — values by COV or polling, device health on every rescan.
//!
//! Each cycle rediscovers, so a device that comes back is picked up and one
//! that stops answering drops to `bacnet.device.up 0` rather than vanishing.

use super::follow::{self, Follow, Point, Sink};
use crate::Result;
use crate::args::{Net, seconds};
use bacnet_probe::scan::{self, Options};
use bacnet_probe::{Client, Device, metrics, names};
use clap::Args;
use std::time::{Duration, Instant};

#[derive(Args)]
pub(crate) struct ExportArgs {
    /// Seconds between polls of points that do not support COV.
    #[arg(long, default_value = "10", value_parser = seconds)]
    every: Duration,
    /// Seconds between rediscovering devices and rereading their objects.
    #[arg(long, default_value = "300", value_parser = seconds)]
    rescan: Duration,
    /// Only this device instance.
    #[arg(long)]
    device: Option<u32>,
    /// Poll everything; do not subscribe to COV.
    #[arg(long)]
    no_cov: bool,
    /// Cap on objects read per device.
    #[arg(long, default_value_t = 2000)]
    max_objects: u32,
}

pub(crate) fn run(client: &mut Client, net: &Net, args: &ExportArgs) -> Result {
    let mut known: Vec<Device> = Vec::new();
    loop {
        let cycle = Instant::now();
        let found: Vec<Device> = super::discover(client, net)?
            .into_iter()
            .filter(|d| args.device.is_none_or(|i| i == d.instance))
            .collect();
        metrics::record_discovery(client.meter(), found.len());
        for gone in known
            .iter()
            .filter(|k| !found.iter().any(|f| f.instance == k.instance))
        {
            eprintln!("device {} stopped answering", gone.instance);
            let identity = client.identity_or(gone.instance, "");
            metrics::record_device(client.meter(), gone, &identity, false, None);
        }
        let mut points = Vec::new();
        let options = Options {
            detail: false,
            max_objects: args.max_objects,
            only: None,
        };
        for device in &found {
            let report = scan::device(client, device, &options, |_, _| {});
            let name = report.this.name();
            let identity = client.identity_or(device.instance, &name);
            metrics::record_device(
                client.meter(),
                device,
                &identity,
                report.problem.is_none() || !report.objects.is_empty(),
                report.object_count,
            );
            for o in &report.objects {
                metrics::record_object(client.meter(), &identity, o);
            }
            points.extend(
                report
                    .objects
                    .iter()
                    .filter(|o| names::is_point(o.id.kind) && o.present_value().is_some())
                    .map(|o| Point::new(device, &name, o, false)),
            );
            eprintln!(
                "device {} {name:?}: {} objects, {} points",
                device.instance,
                report.objects.len(),
                points.len()
            );
        }
        known = found;
        let until = cycle + args.rescan;
        if points.is_empty() {
            std::thread::sleep(until.saturating_duration_since(Instant::now()));
            continue;
        }
        let lifetime = u32::try_from(args.rescan.as_secs())
            .unwrap_or(u32::MAX)
            .saturating_add(60);
        let how = Follow {
            every: args.every,
            cov: !args.no_cov,
            lifetime,
            sink: Sink::Quiet,
            until: Some(until),
        };
        follow::follow(client, &mut points, &how)?;
    }
}
