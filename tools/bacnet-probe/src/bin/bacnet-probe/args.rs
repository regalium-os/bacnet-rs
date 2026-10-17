// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The command line: global network and telemetry options, and the
//! subcommands. Options are accepted before or after the subcommand.

use crate::cli;
use bacnet_probe::Scope;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

#[derive(Parser)]
#[command(name = "bacnet-probe", version, about = "Discover, browse, watch, change and export BACnet/IP devices", after_help = crate::help::EXAMPLES)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Cmd>,
    #[command(flatten)]
    pub(crate) net: Net,
    #[command(flatten)]
    pub(crate) telemetry: Telemetry,
}

/// Where and how to talk `BACnet`/IP. Equality is what the TUI's settings
/// tab compares to decide whether an edit needs the socket rebound.
#[derive(Args, Clone, Debug, PartialEq)]
pub(crate) struct Net {
    /// Where Who-Is goes: a broadcast (subnet .255) or one device/router IP.
    #[arg(long, global = true, default_value = "255.255.255.255", value_parser = parse_ipv4)]
    pub(crate) target: Ipv4Addr,
    /// Sweep these ranges instead of --target, and accept answers only from
    /// them: 192.168.*, 10.1.0.0/16, or one address. Repeatable.
    #[arg(long, global = true, value_name = "RANGE")]
    pub(crate) scan: Vec<Scope>,
    /// With --scan: one Who-Is per host instead of per /24 broadcast, for
    /// routers that drop directed broadcasts.
    #[arg(long, global = true)]
    pub(crate) unicast: bool,
    /// With --scan: datagrams per second.
    #[arg(long, global = true, default_value_t = 500)]
    pub(crate) rate: u32,
    /// Register as a foreign device with this BBMD (IP or IP:port) and
    /// discover through it.
    #[arg(long, global = true, value_name = "ADDR", value_parser = parse_bbmd)]
    pub(crate) bbmd: Option<SocketAddr>,
    /// Foreign device registration time-to-live, seconds; renewed before it lapses.
    #[arg(long, global = true, default_value_t = 300)]
    pub(crate) ttl: u16,
    /// `BACnet`/IP UDP port.
    #[arg(long, global = true, default_value_t = 47808)]
    pub(crate) port: u16,
    /// Local socket [default: 0.0.0.0:<port>, else an ephemeral port].
    #[arg(long, global = true, value_parser = parse_bind)]
    pub(crate) bind: Option<SocketAddr>,
    /// Seconds to collect I-Am replies.
    #[arg(long, global = true, default_value = "3", value_parser = seconds)]
    pub(crate) wait: Duration,
    /// Seconds to wait for each reply before retrying [default: 2, or 1 in
    /// the TUI, where a dead device should cost a second].
    #[arg(long, global = true, value_parser = seconds)]
    pub(crate) timeout: Option<Duration>,
    /// Dissect every packet sent and received to stderr (subcommands only).
    #[arg(long, global = true)]
    pub(crate) explain: bool,
}

/// Where metrics go.
#[derive(Args, Clone, Debug)]
pub(crate) struct Telemetry {
    /// Export metrics over OTLP gRPC to HOST:PORT, from any command. Without
    /// it only `export` exports, to telemetry.toml / TELEMETRY_* / the SDK default.
    #[arg(long, global = true, value_name = "HOST:PORT", value_parser = parse_otlp)]
    pub(crate) otlp: Option<(String, u16)>,
    /// A resource label on every exported series, e.g. --label site=tower-a. Repeatable.
    #[arg(long, global = true, value_name = "KEY=VALUE", value_parser = parse_label)]
    pub(crate) label: Vec<(String, String)>,
}

#[derive(Subcommand)]
pub(crate) enum Cmd {
    /// Interactive browser (the default when no subcommand is given).
    Tui {
        /// Seconds between polls of points that do not support COV.
        #[arg(long, default_value = "2", value_parser = seconds)]
        interval: Duration,
    },
    /// List the devices that answer Who-Is.
    Whois(cli::whois::WhoisArgs),
    /// Find which device holds an object, by name or TYPE:N (Who-Has).
    Find(cli::whois::FindArgs),
    /// Read properties of one object.
    Read(cli::read::ReadArgs),
    /// Discover, then read every object of every device.
    Scan(cli::scan::ScanArgs),
    /// Follow present-values and print each change: COV where the device
    /// allows it, polled where not.
    Watch(cli::watch::WatchArgs),
    /// Run headless: every point of every device, continuously, as metrics.
    Export(cli::export::ExportArgs),
    /// Dissect a hex-dumped datagram, offline (stdin if none given).
    Dissect(cli::dissect::DissectArgs),
    /// Read a device, object or property by resource name.
    Get(cli::resource::NameArgs),
    /// List devices (discovering them), a device's objects, or an object's properties.
    List(cli::resource::NameArgs),
    /// Write a property (`WriteProperty`), typed as the property holds it.
    Write(cli::resource::WriteArgs),
    /// Create an object in a device (`CreateObject`).
    Create(cli::resource::CreateArgs),
    /// Delete an object from a device (`DeleteObject`).
    Delete(cli::resource::NameArgs),
}

/// `table` for people, `json` for programs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub(crate) enum Format {
    #[default]
    Table,
    Json,
}

pub(crate) fn seconds(s: &str) -> Result<Duration, String> {
    let secs: f64 = s.parse().map_err(|e| format!("{e}"))?;
    Duration::try_from_secs_f64(secs).map_err(|e| e.to_string())
}

/// The options exactly as they are when none is given, from clap itself so
/// the TUI's "reset to defaults" cannot drift from `--help`.
pub(crate) fn defaults() -> Net {
    Cli::parse_from(["bacnet-probe"]).net
}

/// An IPv4 address. `BACnet`/IPv6 is not this datalink with a wider address:
/// it is Annex U, with no broadcast and a virtual MAC layer, so an IPv6
/// address is refused by name rather than half-used.
pub(crate) fn parse_ipv4(s: &str) -> Result<Ipv4Addr, String> {
    s.trim().parse().map_err(|_| not_ipv4(s, "an IPv4 address"))
}

/// A local IPv4 socket, `0.0.0.0:47808`.
pub(crate) fn parse_bind(s: &str) -> Result<SocketAddr, String> {
    s.trim()
        .parse::<SocketAddr>()
        .ok()
        .filter(SocketAddr::is_ipv4)
        .ok_or_else(|| not_ipv4(s, "an IPv4 IP:PORT, e.g. 0.0.0.0:47808"))
}

/// A BBMD: `IP` (port 47808) or `IP:PORT`, IPv4.
pub(crate) fn parse_bbmd(s: &str) -> Result<SocketAddr, String> {
    let s = s.trim();
    s.parse::<SocketAddr>()
        .ok()
        .or_else(|| s.parse::<Ipv4Addr>().ok().map(|ip| (ip, 47808).into()))
        .filter(SocketAddr::is_ipv4)
        .ok_or_else(|| not_ipv4(s, "an IPv4 address, optionally with :port"))
}

fn not_ipv4(s: &str, expected: &str) -> String {
    let bare = s
        .trim()
        .trim_start_matches('[')
        .split(']')
        .next()
        .unwrap_or_default();
    if bare.parse::<Ipv6Addr>().is_ok() {
        format!("{s:?} is IPv6; BACnet/IPv6 (Annex U) is not supported yet, expected {expected}")
    } else {
        format!("{s:?}: expected {expected}")
    }
}

fn parse_otlp(s: &str) -> Result<(String, u16), String> {
    let (host, port) = s
        .rsplit_once(':')
        .ok_or_else(|| format!("{s:?}: expected HOST:PORT, e.g. localhost:4317"))?;
    Ok((
        host.to_owned(),
        port.parse()
            .map_err(|_| format!("{port:?} is not a port"))?,
    ))
}

fn parse_label(s: &str) -> Result<(String, String), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| format!("{s:?}: expected KEY=VALUE"))?;
    Ok((k.to_owned(), v.to_owned()))
}
