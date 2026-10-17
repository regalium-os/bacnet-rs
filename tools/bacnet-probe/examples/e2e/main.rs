// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! An end-to-end loop against a live simulated gateway, sent to an OTLP
//! collector, so traces, logs and metrics can be read as one story.
//!
//! Every cycle is one trace, `bacnet.e2e.cycle`, holding a span per step —
//! discover, poll, write, override, refusal, lifecycle, cov — each holding
//! the API calls it made, each holding the `BACnet` requests those made. Logs
//! come at every level and carry the span they were written in; metrics are
//! the series `bacnet-probe export` produces, fed by mock values that move.
//!
//! ```sh
//! cargo run -p regalium-bacnet-probe --example e2e -- localhost:6009 [cycles] [run-label]
//! ```

mod change;
mod steps;

use bacnet_probe::api::Api;
use bacnet_probe::frame::Delivery;
use bacnet_probe::sim::Sim;
use bacnet_probe::{Client, Discovery};
use regalium_bacnet_telemetry::{Attribute, Level, Logger, Meter, Tracer, keys};
use regalium_bacnet_telemetry_otel::Otel;
use std::sync::Arc;
use std::time::Duration;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result {
    let mut args = std::env::args().skip(1);
    let target = args.next().unwrap_or_else(|| "localhost:4317".into());
    let cycles: u32 = args.next().and_then(|n| n.parse().ok()).unwrap_or(10);
    let run = args.next().unwrap_or_else(|| "e2e".into());
    let (host, port) = target.rsplit_once(':').ok_or("expected HOST:PORT")?;

    let gateway = Sim::lift_gateway().live().spawn()?;
    let otel = Arc::new(
        Otel::builder("bacnet-e2e", env!("CARGO_PKG_VERSION"))
            .otlp(host, port.parse()?)
            .label("run", &run)
            .log_level(Level::Debug)
            .build()?,
    );
    let client = Client::bind("127.0.0.1:0".parse()?)?
        .with_timeout(Duration::from_millis(500))
        .with_retries(1)
        .with_meter(otel.clone() as Arc<dyn Meter>)
        .with_tracer(otel.clone() as Arc<dyn Tracer>)
        .with_logger(otel.clone() as Arc<dyn Logger>);
    let plan = Discovery::to(gateway, Delivery::Unicast).with_wait(Duration::from_millis(300));
    let mut api = Api::new(client, plan.clone());

    println!(
        "e2e: {cycles} cycles against a simulated gateway at {gateway}, exporting to {target} (run={run})"
    );
    let mut failed = 0;
    for cycle in 1..=cycles {
        let span = api.client().enter("bacnet.e2e.cycle");
        api.client().set(Attribute::int(keys::CYCLE, cycle.into()));
        api.client().log(
            Level::Info,
            &format_args!("cycle {cycle} of {cycles}: start"),
            &[],
        );
        let result = steps::cycle(&mut api, &plan, cycle);
        match &result {
            Ok(summary) => {
                api.client()
                    .log(Level::Info, &format_args!("cycle {cycle}: {summary}"), &[]);
            }
            Err(e) => {
                failed += 1;
                api.client().log(
                    Level::Error,
                    &format_args!("cycle {cycle} failed: {e}"),
                    &[],
                );
            }
        }
        let error = result.as_ref().err().map(ToString::to_string);
        api.client()
            .exit(span, error.as_ref().map(|e| e as &dyn std::fmt::Display));
        println!(
            "cycle {cycle}: {}",
            result.unwrap_or_else(|e| format!("FAILED: {e}"))
        );
        std::thread::sleep(Duration::from_secs(2));
    }
    otel.flush()?;
    println!("e2e: done, {failed} of {cycles} cycles failed; flushed to {target}");
    if failed > 0 {
        Err(format!("{failed} cycle(s) failed").into())
    } else {
        Ok(())
    }
}
