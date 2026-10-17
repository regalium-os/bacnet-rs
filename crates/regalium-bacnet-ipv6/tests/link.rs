// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! Two BACnet/IPv6 links on `[::1]`, each other's "multicast group": a
//! unicast to a virtual MAC nobody has resolved goes out once the address
//! is learnt, and a broadcast arrives as one.

use regalium_bacnet_core::datalink::{Datalink, Recipient};
use regalium_bacnet_ipv6::BacnetIpv6;
use regalium_bacnet_ipv6::bvll::Vmac;
use std::net::{Ipv6Addr, SocketAddrV6};
use std::time::Duration;

const A: Vmac = Vmac([0, 0, 1]);
const B: Vmac = Vmac([0, 0, 2]);

fn pair() -> (BacnetIpv6, BacnetIpv6) {
    let any = SocketAddrV6::new(Ipv6Addr::LOCALHOST, 0, 0, 0);
    let a = BacnetIpv6::bind(any, A).unwrap();
    let b = BacnetIpv6::bind(any, B).unwrap();
    let (a_at, b_at) = (a.local_addr().unwrap(), b.local_addr().unwrap());
    (a.group(b_at), b.group(a_at))
}

#[test]
fn a_unicast_to_an_unresolved_vmac_is_held_resolved_and_delivered() {
    let (a, b) = pair();
    let listener = std::thread::spawn(move || {
        let mut buf = [0u8; 1500];
        let mut got = Vec::new();
        for _ in 0..5 {
            if let Some(r) = b.receive(&mut buf, Duration::from_millis(400)).unwrap() {
                got.push((r.from, r.broadcast, buf[..r.len].to_vec()));
            }
        }
        got
    });
    a.send(Recipient::Unicast(B), &[0x01, 0x00, 0x10, 0x08])
        .unwrap();
    // A hears B's Address-Resolution-ACK inside receive, and sends what it held.
    assert!(
        a.receive(&mut [0; 1500], Duration::from_millis(400))
            .unwrap()
            .is_none()
    );
    a.send(Recipient::LocalBroadcast, &[0x01, 0x00, 0x10, 0x00])
        .unwrap();
    let got = listener.join().unwrap();
    assert_eq!(
        got,
        [
            (A, false, vec![0x01, 0x00, 0x10, 0x08]),
            (A, true, vec![0x01, 0x00, 0x10, 0x00])
        ]
    );
}
