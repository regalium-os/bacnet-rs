// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! One reading of a followed point: recorded always, printed on change.

use super::follow::{Point, Sink};
use super::json_out;
use bacnet_probe::metrics;
use bacnet_probe::{Client, Value, explain, names};
use std::time::Instant;

/// Records every reading; prints only changes and losses of contact.
pub(crate) fn report(
    client: &Client,
    p: &mut Point,
    now: std::result::Result<Value, String>,
    sink: Sink,
    started: Instant,
) {
    let device = client.identity_or(p.device.instance, &p.device_name);
    metrics::record_value(
        client.meter(),
        &device,
        p.object,
        &p.name,
        p.units,
        now.as_ref().ok(),
    );
    match (&now, &p.last) {
        (Ok(v), Some(last)) if v == last => return,
        (Err(_), None) => return,
        _ => {}
    }
    let t = started.elapsed().as_secs_f32();
    match sink {
        Sink::Json => {
            let to = now.as_ref().map_err(Clone::clone);
            println!(
                "{}",
                json_out::event(t, p.device.instance, p.object, &p.name, p.last.as_ref(), to)
                    .compact()
            );
        }
        Sink::Table => {
            let show = |v: &Value| explain::present(p.object.kind, v, p.units, &p.labels);
            let was = p.last.as_ref().map_or_else(|| "-".into(), show);
            let to = now.as_ref().map_or_else(|e| format!("offline ({e})"), show);
            let via = if p.process.is_some() { "cov " } else { "poll" };
            println!(
                "{t:>8.1}s {via} {:<8} {:<24} {:<30} {was} -> {to}",
                p.device.instance,
                names::object(p.object),
                p.name
            );
        }
        Sink::Quiet => {}
    }
    p.last = now.ok();
}
