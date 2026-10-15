// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `bacnet-probe scan`: discover, then read every object of every device.

use super::follow::{self, Follow, Point, Sink};
use super::{json_out, parse_object, table};
use crate::Result;
use crate::args::{Format, Net, seconds};
use bacnet_probe::metrics::{self, DeviceLabels};
use bacnet_probe::scan::{self, DeviceReport, Options};
use bacnet_probe::{Client, ObjectId, names};
use clap::Args;
use std::io::{IsTerminal, Write};
use std::time::Duration;

#[derive(Args)]
pub(crate) struct ScanArgs {
    /// Only this device instance.
    #[arg(long)]
    device: Option<u32>,
    /// Only this object, every property (e.g. analog-input:1).
    #[arg(long, value_parser = parse_object)]
    object: Option<ObjectId>,
    /// Every standard property of every object.
    #[arg(long)]
    detail: bool,
    /// Cap on objects read per device.
    #[arg(long, default_value_t = 100)]
    max_objects: u32,
    #[arg(long, value_enum, default_value_t)]
    format: Format,
    /// Then follow present-values (COV, else polled every SECS) and print changes.
    #[arg(long, value_parser = seconds)]
    watch: Option<Duration>,
}

pub(crate) fn run(client: &mut Client, net: &Net, args: &ScanArgs) -> Result {
    let devices = super::discover(client, net)?;
    metrics::record_discovery(client.meter(), devices.len());
    let options = Options {
        detail: args.detail,
        max_objects: args.max_objects,
        only: args.object,
    };
    let reports: Vec<DeviceReport> = devices
        .iter()
        .filter(|d| args.device.is_none_or(|i| i == d.instance))
        .map(|d| {
            let report = scan::device(client, d, &options, |i, of| progress(d.instance, i, of));
            progress(d.instance, 0, 0);
            record(client, &report);
            report
        })
        .collect();
    match args.format {
        Format::Json => println!("{}", json_out::document(&reports).pretty()),
        Format::Table => print_tables(&reports, args.detail || args.object.is_some()),
    }
    let Some(every) = args.watch else {
        return Ok(());
    };
    let mut points: Vec<Point> = reports
        .iter()
        .flat_map(|r| r.objects.iter().map(move |o| (r, o)))
        .filter(|(_, o)| names::is_point(o.id.kind) && o.present_value().is_some())
        .map(|(r, o)| Point::new(&r.device, &r.this.name(), o, false))
        .collect();
    if points.is_empty() {
        eprintln!("Nothing to watch: no readable points found.");
        return Ok(());
    }
    let sink = if args.format == Format::Json {
        Sink::Json
    } else {
        Sink::Table
    };
    follow::follow(
        client,
        &mut points,
        &Follow {
            every,
            cov: true,
            lifetime: 120,
            sink,
            until: None,
        },
    )
}

/// Everything a scan read, as metrics (a no-op without `--otlp`).
fn record(client: &Client, report: &DeviceReport) {
    let name = report.this.name();
    metrics::record_device(
        client.meter(),
        &report.device,
        Some(&report.this),
        true,
        report.object_count,
    );
    for o in &report.objects {
        metrics::record_object(
            client.meter(),
            DeviceLabels {
                instance: report.device.instance,
                name: &name,
            },
            o,
        );
    }
}

fn print_tables(reports: &[DeviceReport], detail: bool) {
    if reports.is_empty() {
        return;
    }
    print!("{}", table::devices(reports));
    for r in reports {
        let count = r
            .object_count
            .map_or_else(String::new, |n| format!(", {n} objects"));
        println!("\ndevice {} {}{count}", r.device.instance, r.this.name());
        if detail {
            print!("{}", table::properties(&r.this));
            for o in &r.objects {
                println!("\n{} {}", names::object(o.id), o.name());
                print!("{}", table::properties(o));
            }
        } else if !r.objects.is_empty() {
            print!("{}", table::objects(r));
        }
        if let Some(problem) = &r.problem {
            println!("problem: {problem}");
        }
    }
}

/// `device 599: object 12/120` on one rewritten stderr line, terminals only.
fn progress(instance: u32, i: u32, of: u32) {
    let mut err = std::io::stderr();
    if !err.is_terminal() {
        return;
    }
    let _ = if of == 0 {
        write!(err, "\r\x1b[2K")
    } else {
        write!(err, "\r\x1b[2Kdevice {instance}: object {i}/{of}")
    };
    let _ = err.flush();
}
