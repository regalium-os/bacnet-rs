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
mod tui;

use args::{Cli, Cmd, Net, Telemetry};
use bacnet_probe::Client;
use clap::Parser;
use regalium_bacnet_telemetry::{Meter, NoopMeter};
use regalium_bacnet_telemetry_otel::Otel;
use std::net::SocketAddr;
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
    // Short timeouts: in a UI, a dead device should cost a second, not six.
    let mut client = bind(net, false)?
        .with_timeout(Duration::from_secs(1))
        .with_retries(1)
        .with_meter(meter);
    register(&mut client, net)?;
    let label = if net.scan.is_empty() {
        net.target.to_string()
    } else {
        cli::scopes(&net.scan)
    };
    Ok(tui::run(client, cli::plan(net)?, label, interval)?)
}

fn connect(net: &Net, meter: Arc<dyn Meter>) -> Result<Client> {
    let mut client = bind(net, true)?.with_timeout(net.timeout).with_meter(meter);
    if net.explain {
        client = client.with_trace(cli::print_packet);
    }
    register(&mut client, net)?;
    Ok(client)
}

/// Registers with `--bbmd`, if given, before anything is sent.
fn register(client: &mut Client, net: &Net) -> Result {
    if let Some(bbmd) = net.bbmd {
        client
            .register_foreign_device(bbmd, net.ttl)
            .map_err(|e| format!("--bbmd {bbmd}: {e}"))?;
        eprintln!(
            "registered as a foreign device with {bbmd} for {} s",
            net.ttl
        );
    }
    Ok(())
}

fn bind(net: &Net, chatty: bool) -> std::io::Result<Client> {
    if let Some(addr) = net.bind {
        return Client::bind(addr);
    }
    // Many devices broadcast I-Am to port 47808, so prefer to own it.
    Client::bind(SocketAddr::from(([0, 0, 0, 0], net.port))).or_else(|e| {
        if chatty {
            eprintln!("note: cannot bind :{} ({e}); using an ephemeral port, so broadcast I-Ams may be missed", net.port);
        }
        Client::bind(SocketAddr::from(([0, 0, 0, 0], 0)))
    })
}
