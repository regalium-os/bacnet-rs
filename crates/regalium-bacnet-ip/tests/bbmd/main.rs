// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

//! The BBMD's forwarding rules (Annex J.4.5, `forwarding.rs`) and tables
//! (J.5.2, `tables.rs`), on a fake clock, with every octet it sends spelled
//! out. The helpers and the topology they share are here.

use regalium_bacnet_ip::bbmd::{Bbmd, Handled, Target};
use regalium_bacnet_ip::bvll::{BdtEntry, Bvll};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

pub(crate) fn at(a: u8, b: u8, c: u8, d: u8) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(a, b, c, d), 47808)
}

pub(crate) fn octets(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

/// This BBMD on 192.168.1.0/24, two-hop; a two-hop peer; a one-hop peer.
pub(crate) fn bbmd() -> Bbmd {
    let all = Ipv4Addr::BROADCAST;
    Bbmd::new(
        at(192, 168, 1, 1),
        vec![
            BdtEntry {
                address: at(192, 168, 1, 1),
                mask: all,
            },
            BdtEntry {
                address: at(192, 168, 2, 1),
                mask: all,
            },
            BdtEntry {
                address: at(192, 168, 3, 1),
                mask: Ipv4Addr::new(255, 255, 255, 0),
            },
        ],
    )
}

pub(crate) fn handle(b: &mut Bbmd, from: SocketAddrV4, msg: &str, secs: u64) -> Handled {
    let m = octets(msg);
    b.handle(from, &Bvll::decode(&m).unwrap(), Duration::from_secs(secs))
}

pub(crate) fn sent(h: &Handled) -> Vec<(Target, Vec<u8>)> {
    h.out.iter().map(|o| (o.to, o.bvll.clone())).collect()
}

pub(crate) const WHO_IS: &str = "01 20 ff ff 00 ff 10 08";
pub(crate) const FD: (u8, u8, u8, u8) = (10, 0, 0, 5);

mod forwarding;
mod tables;
