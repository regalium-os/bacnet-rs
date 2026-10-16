// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The editable settings, one per network option: how each is shown as
//! text, and how text becomes the option again. Parsing goes through the
//! command line's own parsers, so a value the tab accepts is exactly one the
//! flag would, and the two cannot drift apart.

use crate::args::{self, Net};
use bacnet_probe::Scope;
use std::time::Duration;

/// One setting. The order of [`FIELDS`] is the order on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    Target,
    Port,
    Bind,
    Scan,
    Unicast,
    Rate,
    Bbmd,
    Ttl,
    Wait,
    Timeout,
    Interval,
}

pub(crate) const FIELDS: [Field; 11] = [
    Field::Target,
    Field::Port,
    Field::Bind,
    Field::Scan,
    Field::Unicast,
    Field::Rate,
    Field::Bbmd,
    Field::Ttl,
    Field::Wait,
    Field::Timeout,
    Field::Interval,
];

/// The poll interval's bounds, the same the `+` and `-` keys clamp to.
pub(crate) const INTERVAL: (Duration, Duration) =
    (Duration::from_millis(250), Duration::from_secs(60));

impl Field {
    /// The name on screen, and the flag it mirrors.
    pub(crate) fn label(self) -> (&'static str, &'static str) {
        match self {
            Self::Target => ("Who-Is target", "--target"),
            Self::Port => ("UDP port", "--port"),
            Self::Bind => ("Local socket", "--bind"),
            Self::Scan => ("Scan ranges", "--scan"),
            Self::Unicast => ("Unicast sweep", "--unicast"),
            Self::Rate => ("Sweep rate /s", "--rate"),
            Self::Bbmd => ("BBMD", "--bbmd"),
            Self::Ttl => ("Registration TTL s", "--ttl"),
            Self::Wait => ("I-Am wait s", "--wait"),
            Self::Timeout => ("Reply timeout s", "--timeout"),
            Self::Interval => ("Poll interval s", "--interval"),
        }
    }

    /// What it does, and what an empty value means.
    pub(crate) fn help(self) -> &'static str {
        match self {
            Self::Target => {
                "Where Who-Is goes when no range is scanned: a subnet broadcast (x.x.x.255, Annex J.4.5) or one device or router. IPv4 only: BACnet/IPv6 is Annex U, not yet supported."
            }
            Self::Port => {
                "The BACnet/IP UDP port, 47808 (0xBAC0) unless the site moved it. Also the local port when Local socket is empty."
            }
            Self::Bind => {
                "Local IP:PORT to listen on. Empty: 0.0.0.0 on the UDP port, else an ephemeral port, which misses I-Ams broadcast to 47808."
            }
            Self::Scan => {
                "Comma-separated ranges to sweep instead of the target, and the only places answers are accepted from: 192.168.*, 10.1.0.0/16, one address. Empty: none."
            }
            Self::Unicast => {
                "Sweep one datagram per host rather than one broadcast per /24, for routers that drop directed broadcasts. Space toggles."
            }
            Self::Rate => {
                "Datagrams per second while sweeping. 500 is gentle enough for a network shared with life-safety equipment."
            }
            Self::Bbmd => {
                "Register as a foreign device (Annex J.5) with this BBMD, IP or IP:PORT, to hear and reach another subnet's broadcasts. Empty: none."
            }
            Self::Ttl => {
                "Foreign device registration lifetime. Renewed before it lapses, inside the BBMD's 30 s grace (J.5.2.3)."
            }
            Self::Wait => "How long discovery listens for I-Am after the last Who-Is.",
            Self::Timeout => {
                "How long to wait for each reply before retrying once. Empty: 1 s, so a dead device costs a second."
            }
            Self::Interval => {
                "How often points the device will not subscribe by COV are polled. Also changed by + and -."
            }
        }
    }

    /// Whether it is on or off rather than typed.
    pub(crate) fn is_toggle(self) -> bool {
        self == Self::Unicast
    }

    /// The value as the form shows it; empty for an absent option.
    pub(crate) fn show(self, net: &Net, interval: Duration) -> String {
        let secs = |d: Duration| d.as_secs_f64().to_string();
        match self {
            Self::Target => net.target.to_string(),
            Self::Port => net.port.to_string(),
            Self::Bind => net.bind.map(|a| a.to_string()).unwrap_or_default(),
            Self::Scan => crate::cli::scopes(&net.scan),
            Self::Unicast => if net.unicast { "on" } else { "off" }.into(),
            Self::Rate => net.rate.to_string(),
            Self::Bbmd => net.bbmd.map(|a| a.to_string()).unwrap_or_default(),
            Self::Ttl => net.ttl.to_string(),
            Self::Wait => secs(net.wait),
            Self::Timeout => net.timeout.map(secs).unwrap_or_default(),
            Self::Interval => secs(interval),
        }
    }

    /// Reads `text` into the option it edits.
    pub(crate) fn parse(
        self,
        text: &str,
        net: &mut Net,
        interval: &mut Duration,
    ) -> Result<(), String> {
        let text = text.trim();
        let optional = (!text.is_empty()).then_some(text);
        match self {
            Self::Target => net.target = args::parse_ipv4(text)?,
            Self::Port => net.port = number(text)?,
            Self::Bind => net.bind = optional.map(args::parse_bind).transpose()?,
            Self::Scan => {
                net.scan = text
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::parse::<Scope>)
                    .collect::<Result<_, _>>()?;
            }
            Self::Unicast => net.unicast = text == "on",
            Self::Rate => net.rate = number::<u32>(text)?.max(1),
            Self::Bbmd => net.bbmd = optional.map(args::parse_bbmd).transpose()?,
            Self::Ttl => net.ttl = number(text)?,
            Self::Wait => net.wait = args::seconds(text)?,
            Self::Timeout => {
                let d = optional.map(args::seconds).transpose()?;
                if d.is_some_and(|d| d.is_zero()) {
                    return Err("a zero timeout would never see a reply".into());
                }
                net.timeout = d;
            }
            Self::Interval => {
                let d = args::seconds(text)?;
                if d < INTERVAL.0 || d > INTERVAL.1 {
                    return Err(format!("{text:?}: between 0.25 and 60 seconds"));
                }
                *interval = d;
            }
        }
        Ok(())
    }
}

fn number<T: std::str::FromStr>(text: &str) -> Result<T, String> {
    text.parse()
        .map_err(|_| format!("{text:?} is not a whole number in range"))
}
