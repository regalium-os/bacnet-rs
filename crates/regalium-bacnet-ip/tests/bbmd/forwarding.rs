// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Where each kind of broadcast goes.

use super::{FD, WHO_IS, at, bbmd, handle, octets, sent};
use regalium_bacnet_ip::bbmd::{Handled, Target};

#[test]
fn a_local_broadcast_goes_to_each_peer_by_its_mask_and_to_each_foreign_device() {
    let mut b = bbmd();
    let reg = handle(&mut b, at(10, 0, 0, 5), "81 05 00 06 00 3c", 0);
    assert_eq!(
        sent(&reg),
        [(
            Target::Unicast(at(10, 0, 0, 5)),
            octets("81 00 00 06 00 00")
        )]
    );
    let h = handle(
        &mut b,
        at(192, 168, 1, 20),
        &format!("81 0b 00 0c {WHO_IS}"),
        1,
    );
    let fwd = octets(&format!("81 04 00 12 c0 a8 01 14 ba c0 {WHO_IS}"));
    assert_eq!(
        sent(&h),
        [
            (Target::Unicast(at(192, 168, 2, 1)), fwd.clone()),
            (Target::Unicast(at(192, 168, 3, 255)), fwd.clone()),
            (Target::Unicast(at(FD.0, FD.1, FD.2, FD.3)), fwd),
        ]
    );
    assert_eq!(h.deliver, Some((at(192, 168, 1, 20), true)));
}

#[test]
fn a_forwarded_broadcast_is_rebroadcast_locally_when_two_hop_and_never_back_to_peers() {
    let mut b = bbmd();
    handle(&mut b, at(10, 0, 0, 5), "81 05 00 06 00 3c", 0);
    let h = handle(
        &mut b,
        at(192, 168, 2, 1),
        &format!("81 04 00 12 c0 a8 02 32 ba c0 {WHO_IS}"),
        1,
    );
    let fwd = octets(&format!("81 04 00 12 c0 a8 02 32 ba c0 {WHO_IS}"));
    assert_eq!(
        sent(&h),
        [
            (Target::LocalBroadcast, fwd.clone()),
            (Target::Unicast(at(10, 0, 0, 5)), fwd)
        ]
    );
    assert_eq!(
        h.deliver,
        Some((at(192, 168, 2, 50), true)),
        "the answer goes to the origin"
    );
}

#[test]
fn a_foreign_devices_broadcast_reaches_everyone_but_itself_and_strangers_are_refused() {
    let mut b = bbmd();
    handle(&mut b, at(10, 0, 0, 5), "81 05 00 06 00 3c", 0);
    handle(&mut b, at(10, 0, 0, 6), "81 05 00 06 00 3c", 0);
    let h = handle(&mut b, at(10, 0, 0, 5), &format!("81 09 00 0c {WHO_IS}"), 1);
    let fwd = octets(&format!("81 04 00 12 0a 00 00 05 ba c0 {WHO_IS}"));
    let to: Vec<Target> = sent(&h)
        .into_iter()
        .map(|(t, m)| {
            assert_eq!(m, fwd);
            t
        })
        .collect();
    assert_eq!(
        to,
        [
            Target::LocalBroadcast,
            Target::Unicast(at(192, 168, 2, 1)),
            Target::Unicast(at(192, 168, 3, 255)),
            Target::Unicast(at(10, 0, 0, 6)),
        ]
    );
    let stranger = handle(&mut b, at(10, 0, 0, 7), &format!("81 09 00 0c {WHO_IS}"), 1);
    assert_eq!(
        sent(&stranger),
        [(
            Target::Unicast(at(10, 0, 0, 7)),
            octets("81 00 00 06 00 60")
        )]
    );
}

#[test]
fn its_own_broadcast_goes_out_locally_and_heard_back_does_nothing() {
    let mut b = bbmd().foreign_devices(false);
    let out = b.broadcast(&octets(WHO_IS));
    assert_eq!(out[0].to, Target::LocalBroadcast);
    assert_eq!(out[0].bvll, octets(&format!("81 0b 00 0c {WHO_IS}")));
    assert_eq!(
        out[1].bvll,
        octets(&format!("81 04 00 12 c0 a8 01 01 ba c0 {WHO_IS}"))
    );
    assert_eq!(out.len(), 3, "local, and the two peers");
    assert_eq!(
        handle(
            &mut b,
            at(192, 168, 1, 1),
            &format!("81 0b 00 0c {WHO_IS}"),
            0
        ),
        Handled::default()
    );
    let refused = handle(&mut b, at(10, 0, 0, 5), "81 05 00 06 00 3c", 0);
    assert_eq!(sent(&refused)[0].1, octets("81 00 00 06 00 30"));
}
