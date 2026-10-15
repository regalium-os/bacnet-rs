// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! An IPv4 range to discover in, and to accept answers from:
//! `192.168.*`, `192.168.1.*`, `10.0.0.0/16`, or one address.
//!
//! Two ways to sweep one. Broadcasting to every /24's broadcast address in
//! the range costs one datagram per /24 (256 for a /16) and reaches every
//! device whose router forwards directed broadcasts. Unicasting to every
//! host reaches devices behind routers that do not, at one datagram per
//! address, which is why a sweep is rate-limited. Neither reaches a device
//! that answers with a local broadcast from a subnet the probe is not on;
//! foreign device registration (`--bbmd`) is the fix for that.

use std::fmt;
use std::net::Ipv4Addr;
use std::str::FromStr;

/// The largest unicast sweep accepted: a /12. Beyond that a sweep takes
/// hours at any polite rate, and is almost certainly a typo.
pub const MAX_HOSTS: u64 = 1 << 20;

/// A CIDR block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scope {
    network: u32,
    prefix: u8,
}

impl Scope {
    fn mask(self) -> u32 {
        if self.prefix == 0 {
            0
        } else {
            u32::MAX << (32 - u32::from(self.prefix))
        }
    }

    /// Whether `ip` is inside the block.
    #[must_use]
    pub fn contains(self, ip: Ipv4Addr) -> bool {
        u32::from(ip) & self.mask() == self.network
    }

    /// One broadcast address per /24 inside the block (or the block's own,
    /// when it is smaller), paired with whether it really is a broadcast: a
    /// /32 is one host.
    #[must_use]
    pub fn broadcasts(self) -> Vec<(Ipv4Addr, bool)> {
        match self.prefix {
            32 => vec![(Ipv4Addr::from(self.network), false)],
            24..=31 => vec![(Ipv4Addr::from(self.network | !self.mask()), true)],
            p => (0..1u32 << (24 - p))
                .map(|i| (Ipv4Addr::from(self.network | (i << 8) | 0xFF), true))
                .collect(),
        }
    }

    /// How many host addresses a unicast sweep would visit.
    #[must_use]
    pub fn host_count(self) -> u64 {
        match self.prefix {
            31 | 32 => 1 << (32 - u32::from(self.prefix)),
            p => (1u64 << (32 - u32::from(p))) - 2,
        }
    }

    /// Every host address, network and broadcast addresses excluded.
    pub fn hosts(self) -> impl Iterator<Item = Ipv4Addr> {
        let span = 1u64 << (32 - u32::from(self.prefix));
        let (first, last) = if self.prefix >= 31 {
            (0, span)
        } else {
            (1, span - 1)
        };
        (first..last).filter_map(move |i| {
            u32::try_from(u64::from(self.network) + i)
                .ok()
                .map(Ipv4Addr::from)
        })
    }
}

impl FromStr for Scope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let bad =
            |why: &str| format!("{s:?}: {why}; expected 192.168.*, 192.168.1.0/24 or one address");
        let (address, prefix) = if let Some((a, p)) = s.split_once('/') {
            let p: u8 = p.parse().map_err(|_| bad("the prefix is not a number"))?;
            (a.to_owned(), p)
        } else {
            let parts: Vec<&str> = s.split('.').collect();
            let fixed = parts.iter().take_while(|p| **p != "*").count();
            if parts.iter().skip(fixed).any(|p| *p != "*") || parts.len() > 4 || fixed == 0 {
                return Err(bad("a wildcard must be whole trailing octets"));
            }
            let mut octets: Vec<&str> = parts.into_iter().take(fixed).collect();
            octets.resize(4, "0");
            (
                octets.join("."),
                u8::try_from(fixed * 8).map_err(|_| bad("too many octets"))?,
            )
        };
        let ip: Ipv4Addr = address.parse().map_err(|_| bad("not an IPv4 address"))?;
        if prefix > 32 {
            return Err(bad("a prefix is at most 32"));
        }
        let scope = Self { network: 0, prefix };
        Ok(Self {
            network: u32::from(ip) & scope.mask(),
            prefix,
        })
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", Ipv4Addr::from(self.network), self.prefix)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::Scope;
    use std::net::Ipv4Addr;

    fn scope(s: &str) -> Scope {
        s.parse().unwrap()
    }

    #[test]
    fn a_wildcard_and_its_cidr_spelling_are_the_same_block() {
        assert_eq!(scope("192.168.*"), scope("192.168.0.0/16"));
        assert_eq!(scope("192.168.*.*"), scope("192.168.77.1/16"));
        assert_eq!(scope("10.1.2.*").to_string(), "10.1.2.0/24");
        assert_eq!(scope("10.1.2.3").to_string(), "10.1.2.3/32");
    }

    #[test]
    fn a_misplaced_wildcard_or_a_bad_prefix_is_refused() {
        for bad in [
            "192.*.1.0",
            "*",
            "192.168.1.0/33",
            "192.168.1.x",
            "1.2.3.4.5",
        ] {
            assert!(bad.parse::<Scope>().is_err(), "{bad}");
        }
    }

    #[test]
    fn a_slash_16_is_swept_with_one_broadcast_per_slash_24() {
        let b = scope("192.168.*").broadcasts();
        assert_eq!(b.len(), 256);
        assert_eq!(b.first(), Some(&(Ipv4Addr::new(192, 168, 0, 255), true)));
        assert_eq!(b.last(), Some(&(Ipv4Addr::new(192, 168, 255, 255), true)));
        assert_eq!(
            scope("10.0.0.5").broadcasts(),
            vec![(Ipv4Addr::new(10, 0, 0, 5), false)]
        );
    }

    #[test]
    fn a_host_sweep_skips_the_network_and_broadcast_addresses() {
        let s = scope("192.168.1.0/30");
        assert_eq!(s.host_count(), 2);
        assert_eq!(
            s.hosts().collect::<Vec<_>>(),
            [Ipv4Addr::new(192, 168, 1, 1), Ipv4Addr::new(192, 168, 1, 2)]
        );
        assert_eq!(scope("192.168.*").host_count(), 65_534);
    }

    #[test]
    fn membership_follows_the_prefix() {
        let s = scope("192.168.*");
        assert!(s.contains(Ipv4Addr::new(192, 168, 40, 7)));
        assert!(!s.contains(Ipv4Addr::new(192, 169, 0, 1)));
    }
}
