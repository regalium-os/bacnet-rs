// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `bacnet-probe`: an interactive TUI (the default) and scriptable
//! subcommands over the same `BACnet`/IP client.
//!
//! For the subcommands, stdout carries only the result (table or JSON);
//! progress and `--explain` dissections go to stderr, so
//! `bacnet-probe scan --format json > scan.json` stays clean.
//!
//! Metrics go through the telemetry seam and, when exporting, the SDK
//! adapter: `--otlp` exports from any command, `export` always does.

mod args;
mod cli;
mod connect;
mod tui;

use args::{Cli, Cmd, Net, Telemetry};
use bacnet_probe::Client;
use clap::Parser;
use regalium_bacnet_telemetry::{Meter, NoopMeter};
use regalium_bacnet_telemetry_otel::Otel;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

pub(crate) type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result {
    if let Some(Cmd::Dissect(args)) = &cli.command {
        return cli::dissect::run(args);
    }
    let exporting = cli.telemetry.otlp.is_some() || matches!(cli.command, Some(Cmd::Export(_)));
    let otel = if exporting {
        Some(Arc::new(telemetry(&cli.telemetry)?))
    } else {
        None
    };
    let meter: Arc<dyn Meter> = match &otel {
        Some(o) => Arc::clone(o) as Arc<dyn Meter>,
        None => Arc::new(NoopMeter),
    };
    let net = &cli.net;
    match cli.command {
        None => tui(net, meter, Duration::from_secs(2)),
        Some(Cmd::Tui { interval }) => tui(net, meter, interval),
        Some(Cmd::Whois(a)) => cli::whois::whois(&mut connect(net, meter)?, net, &a),
        Some(Cmd::Find(a)) => cli::whois::find(&mut connect(net, meter)?, net, &a),
        Some(Cmd::Read(a)) => cli::read::read(&mut connect(net, meter)?, net, &a),
        Some(Cmd::Watch(a)) => cli::watch::watch(&mut connect(net, meter)?, net, &a),
        Some(Cmd::Scan(a)) => cli::scan::run(&mut connect(net, meter)?, net, &a),
        Some(Cmd::Export(a)) => cli::export::run(&mut connect(net, meter)?, net, &a),
        Some(Cmd::Dissect(_)) => Ok(()),
    }
}

/// Starts the SDK pipeline. Its console logging stays off: it would write
/// over the TUI and into a CLI's machine-readable stdout.
fn telemetry(t: &Telemetry) -> Result<Otel> {
    let mut builder = Otel::builder("bacnet-probe", env!("CARGO_PKG_VERSION"));
    if let Some((host, port)) = &t.otlp {
        builder = builder.otlp(host, *port);
    }
    for (k, v) in &t.label {
        builder = builder.label(k, v);
    }
    Ok(builder.build()?)
}

fn tui(net: &Net, meter: Arc<dyn Meter>, interval: Duration) -> Result {
    let opened = connect::for_tui(net, Arc::clone(&meter))?;
    Ok(tui::run(
        opened,
        net.clone(),
        interval,
        connect::tui_connector(meter),
    )?)
}

fn connect(net: &Net, meter: Arc<dyn Meter>) -> Result<Client> {
    let timeout = net.timeout.unwrap_or(Duration::from_secs(2));
    let mut notes = Vec::new();
    let mut client = connect::bind(net, &mut notes)?
        .with_timeout(timeout)
        .with_meter(meter);
    for n in &notes {
        eprintln!("note: {n}");
    }
    if net.explain {
        client = client.with_trace(cli::print_packet);
    }
    if let Some(registered) = connect::register(&mut client, net)? {
        eprintln!("{registered}");
    }
    Ok(client)
}
