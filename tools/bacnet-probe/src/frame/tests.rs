// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used)]

use super::{Delivery, Route, bvll, decode, encode};
use std::net::SocketAddr;

#[test]
fn a_routed_i_am_keeps_its_source_network_and_mac() {
    // BVLL unicast, NPDU with SNET 5 / SADR 0x0A, then I-Am's first octets.
    let frame = [
        0x81, 0x0A, 0x00, 0x0D, 0x01, 0x08, 0x00, 0x05, 0x01, 0x0A, 0x10, 0x00, 0xC4,
    ];
    let peer: SocketAddr = "10.0.0.2:47808".parse().unwrap();
    let msg = decode(&frame, peer).unwrap();
    assert_eq!(
        msg.source,
        Some(Route {
            net: 5,
            mac: vec![0x0A]
        })
    );
    assert_eq!(msg.apdu, &[0x10, 0x00, 0xC4]);
}

#[test]
fn a_forwarded_npdu_replies_to_the_original_sender_not_the_bbmd() {
    let frame = [0x81, 0x04, 0x00, 0x0C, 10, 0, 0, 9, 0xBA, 0xC0, 0x01, 0x00];
    let msg = decode(&frame, "10.0.0.1:47808".parse().unwrap()).unwrap();
    assert_eq!(msg.from, "10.0.0.9:47808".parse().unwrap());
}

#[test]
fn a_length_past_the_datagram_is_rejected_rather_than_read() {
    let frame = [0x81, 0x0A, 0x00, 0xFF, 0x01, 0x00];
    assert!(decode(&frame, "10.0.0.1:1".parse().unwrap()).is_none());
}

#[test]
fn a_distribute_broadcast_round_trips_and_says_how_it_arrived() {
    let out = encode(Delivery::Distribute, None, false, &[0x10, 0x08]);
    assert_eq!(out, [0x81, 0x09, 0x00, 0x08, 0x01, 0x00, 0x10, 0x08]);
    let msg = decode(&out, "10.0.0.1:47808".parse().unwrap()).unwrap();
    assert_eq!(msg.delivery, Delivery::Distribute);
}

#[test]
fn a_registration_and_its_result_round_trip() {
    let request = bvll::register_foreign_device(300);
    assert_eq!(request, [0x81, 0x05, 0x00, 0x06, 0x01, 0x2C]);
    assert_eq!(bvll::registration(&request), Some(300));
    assert_eq!(
        bvll::result_code(&bvll::result(bvll::REGISTER_NAK)),
        Some(0x0030)
    );
    assert_eq!(bvll::result_code(&request), None);
}
