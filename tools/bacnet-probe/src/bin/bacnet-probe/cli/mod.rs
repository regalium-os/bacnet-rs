// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The scriptable subcommands. Each one reads into plain data (`scan`), then
//! renders it with `table` or `json_out`; `follow` is the live loop that
//! `watch` and `export` share.

pub(crate) mod dissect;
pub(crate) mod export;
pub(crate) mod follow;
pub(crate) mod json_out;
pub(crate) mod read;
pub(crate) mod report;
pub(crate) mod resource;
pub(crate) mod scan;
pub(crate) mod table;
pub(crate) mod watch;
pub(crate) mod whois;

use crate::args::Net;
use bacnet_probe::frame::Delivery;
use bacnet_probe::{
    Client, Device, Direction, Discovery, ObjectId, Scope, dissect as wire, names, props,
};
use std::net::SocketAddr;

/// The Who-Is plan the network options describe: a sweep of `--scan`
/// ranges, or one datagram to `--target`.
pub(crate) fn plan(net: &Net) -> crate::Result<Discovery> {
    let plan = if net.scan.is_empty() {
        // A .255 address is a broadcast as far as BVLL is concerned (Annex J.4.5).
        let delivery = if net.target.octets()[3] == 255 {
            Delivery::Broadcast
        } else {
            Delivery::Unicast
        };
        Discovery::to(SocketAddr::from((net.target, net.port)), delivery)
    } else {
        Discovery::sweep(&net.scan, net.port, net.unicast)?.with_rate(net.rate)
    };
    Ok(plan.with_wait(net.wait))
}

/// Discovers, saying on stderr what is being asked and of whom.
pub(crate) fn discover(client: &mut Client, net: &Net) -> crate::Result<Vec<Device>> {
    let plan = plan(net)?;
    let to = if net.scan.is_empty() {
        net.target.to_string()
    } else {
        scopes(&net.scan)
    };
    let via = client
        .bbmd()
        .map_or_else(String::new, |b| format!(" via BBMD {b}"));
    eprintln!(
        "Who-Is -> {to}{via} ({} datagram(s)), waiting {:?}",
        plan.len(),
        net.wait
    );
    let devices = client.discover(&plan)?;
    if devices.is_empty() {
        eprintln!(
            "No device answered. Check --target/--scan, firewalls, and that nothing else holds UDP {}.",
            net.port
        );
    }
    Ok(devices)
}

/// Finds one device by instance with a ranged Who-Is, so only it answers.
pub(crate) fn locate(client: &mut Client, net: &Net, instance: u32) -> crate::Result<Device> {
    let found = client.discover(&plan(net)?.with_range(Some((instance, instance))))?;
    found.into_iter().next().ok_or_else(|| {
        format!(
            "device {instance} did not answer Who-Is within {:?}",
            net.wait
        )
        .into()
    })
}

/// `192.168.0.0/16, 10.1.2.0/24`.
pub(crate) fn scopes(scopes: &[Scope]) -> String {
    scopes
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// `--explain`: every datagram, field by field, to stderr.
pub(crate) fn print_packet(direction: Direction, peer: SocketAddr, bytes: &[u8]) {
    let arrow = match direction {
        Direction::Sent => format!(">> {peer}"),
        Direction::Received => format!("<< {peer}"),
    };
    eprintln!("\n{arrow}, {} octets", bytes.len());
    for field in wire::frame(bytes) {
        let mut hex: Vec<String> = field
            .bytes
            .iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect();
        if field.bytes.len() > 8 {
            hex.push("..".into());
        }
        eprintln!(
            "  {:<26} {}{}",
            hex.join(" "),
            "  ".repeat(field.depth),
            field.text
        );
    }
}

/// `analog-input:1`, `0:1`.
pub(crate) fn parse_object(s: &str) -> Result<ObjectId, String> {
    names::parse_object(s)
        .ok_or_else(|| format!("{s:?}: expected TYPE:N, e.g. analog-input:1 or 0:1"))
}

/// `present-value`, `85`.
pub(crate) fn parse_property(s: &str) -> Result<u32, String> {
    props::parse(s)
        .ok_or_else(|| format!("{s:?}: not a known property name; use its number instead"))
}

/// `500-599`, `599`.
pub(crate) fn parse_range(s: &str) -> Result<(u32, u32), String> {
    let parse = |v: &str| v.trim().parse::<u32>().map_err(|e| format!("{v:?}: {e}"));
    match s.split_once('-') {
        Some((low, high)) => Ok((parse(low)?, parse(high)?)),
        None => parse(s).map(|n| (n, n)),
    }
}
