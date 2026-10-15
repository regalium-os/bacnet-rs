// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `watch`: follow one device's points and print every change.

use super::follow::{self, Follow, Point, Sink};
use super::{locate, parse_object};
use crate::Result;
use crate::args::{Format, Net, seconds};
use bacnet_probe::{Client, ObjectId, names, scan};
use clap::Args;
use std::time::Duration;

#[derive(Args)]
pub(crate) struct WatchArgs {
    /// Device instance.
    device: u32,
    /// Objects to watch [default: every point on the device].
    #[arg(value_parser = parse_object)]
    objects: Vec<ObjectId>,
    /// Seconds between polls of points that do not support COV.
    #[arg(long, default_value = "2", value_parser = seconds)]
    every: Duration,
    /// Poll everything; do not subscribe to COV.
    #[arg(long)]
    no_cov: bool,
    /// COV subscription lifetime, seconds. Renewed while watching; a
    /// subscription left behind by Ctrl-C lapses after this long.
    #[arg(long, default_value_t = 120)]
    lifetime: u32,
    #[arg(long, value_enum, default_value_t)]
    format: Format,
}

pub(crate) fn watch(client: &mut Client, net: &Net, args: &WatchArgs) -> Result {
    let device = locate(client, net, args.device)?;
    let me = ObjectId::new(8, device.instance);
    let name = scan::object(client, &device, me, false).name();
    let objects = if args.objects.is_empty() {
        let count = scan::object_count(client, &device)?;
        let mut all = scan::object_ids(client, &device, count)?;
        all.retain(|o| names::is_point(o.kind));
        all
    } else {
        args.objects.clone()
    };
    if objects.is_empty() {
        return Err(format!("device {} has no points to watch", device.instance).into());
    }
    let wanted: Vec<(ObjectId, Vec<u32>)> = objects
        .iter()
        .map(|o| (*o, scan::summary_props(o.kind)))
        .collect();
    let mut points: Vec<Point> = wanted
        .chunks(scan::BATCH)
        .flat_map(|chunk| scan::read_many(client, &device, chunk))
        .map(|o| Point::new(&device, &name, &o, true))
        .collect();
    let sink = if args.format == Format::Json {
        Sink::Json
    } else {
        Sink::Table
    };
    let how = Follow {
        every: args.every,
        cov: !args.no_cov,
        lifetime: args.lifetime,
        sink,
        until: None,
    };
    follow::follow(client, &mut points, &how)
}
