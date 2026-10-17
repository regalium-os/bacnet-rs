// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The foreign device and broadcast distribution tables.

use super::{at, bbmd, handle, octets, sent};
use regalium_bacnet_ip::bbmd::Target;
use regalium_bacnet_ip::bvll::BdtEntry;
use std::net::Ipv4Addr;
use std::time::Duration;

#[test]
fn the_foreign_device_table_counts_down_with_its_grace_and_then_forgets() {
    let mut b = bbmd();
    handle(&mut b, at(10, 0, 0, 5), "81 05 00 06 00 3c", 0);
    let h = handle(&mut b, at(10, 0, 0, 9), "81 06 00 04", 30);
    let ack = octets("81 07 00 0e 0a 00 00 05 ba c0 00 3c 00 3c");
    assert_eq!(sent(&h), [(Target::Unicast(at(10, 0, 0, 9)), ack)]);
    assert_eq!(b.deadline(), Some(Duration::from_secs(90)));
    b.expire(Duration::from_secs(90));
    assert!(b.fdt().is_empty() && b.deadline().is_none());
    let del = handle(&mut b, at(10, 0, 0, 9), "81 08 00 0a 0a 00 00 05 ba c0", 91);
    assert_eq!(sent(&del)[0].1, octets("81 00 00 06 00 50"));
}

#[test]
fn the_table_is_read_freely_but_written_only_when_allowed() {
    let mut b = bbmd();
    let read = handle(&mut b, at(10, 0, 0, 9), "81 02 00 04", 0);
    assert_eq!(
        &sent(&read)[0].1[..4],
        &[0x81, 0x03, 0x00, 0x22],
        "three entries"
    );
    let write = "81 01 00 0e c0 a8 09 01 ba c0 ff ff ff ff";
    assert_eq!(
        sent(&handle(&mut b, at(10, 0, 0, 9), write, 0))[0].1,
        octets("81 00 00 06 00 10")
    );
    let mut open = bbmd().writable_bdt(true);
    assert_eq!(
        sent(&handle(&mut open, at(10, 0, 0, 9), write, 0))[0].1,
        octets("81 00 00 06 00 00")
    );
    assert_eq!(
        open.bdt(),
        [BdtEntry {
            address: at(192, 168, 9, 1),
            mask: Ipv4Addr::BROADCAST
        }]
    );
}
