// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! Segmentation, both halves, over UDP: the driver's device sends an answer
//! too large for the client in segments, and the driver's client
//! acknowledges and reassembles them.

use regalium_bacnet::apdu::{Limits, Segments};
use regalium_bacnet::ip::BacnetIp;
use regalium_bacnet::obj::Memory;
use regalium_bacnet::primitive::{ObjectIdentifier, Primitive};
use regalium_bacnet::runtime::{Config, Error, RequestError};
use regalium_bacnet::{Client, Server, tsm};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

const DEVICE: ObjectIdentifier = ObjectIdentifier {
    object_type: 8,
    instance: 599,
};

/// A device with thirty analog values, so its object list is 155 octets of
/// identifiers; and a client that takes `limits` and, if `segmented`,
/// reassembles.
fn pair(segmented: bool) -> Client<BacnetIp> {
    let loopback = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0);
    let client_link = BacnetIp::bind(loopback).unwrap();
    let device_link = BacnetIp::bind(loopback)
        .unwrap()
        .broadcast_address(client_link.local_addr());
    let device_addr = device_link.local_addr();
    let mut db = Memory::new(599, "Big gateway", 260);
    for i in 1..=30 {
        db = db.object(
            ObjectIdentifier {
                object_type: 2,
                instance: i,
            },
            &format!("Value {i}"),
        );
    }
    let mut server = Server::new(device_link, db);
    std::thread::spawn(move || server.serve(Duration::from_secs(3)).unwrap());
    // 50-octet APDUs: the smallest any device takes.
    let timers = tsm::Config::default()
        .accept(Limits::new(Segments::MoreThan64, 0))
        .segmented_response(segmented);
    let mut client = Client::new(client_link, Config::default().transactions(timers));
    client
        .who_is_at(device_addr, Some((599, 599)), Duration::from_millis(300))
        .unwrap();
    client
}

#[test]
fn an_answer_too_large_for_the_client_arrives_in_segments_and_is_reassembled() {
    let mut client = pair(true);
    let reading = client.read_property(599, DEVICE, 76).unwrap();
    // The device, then thirty analog values, each a five-octet identifier.
    assert_eq!(reading.value.len(), 31 * 5);
    let first = &reading.value[..5];
    assert_eq!(first, [0xc4, 0x02, 0x00, 0x02, 0x57]);
    assert_eq!(
        reading.property().unwrap().name,
        "devices/599/objects/device-599/properties/object-list"
    );
}

#[test]
fn a_client_that_does_not_reassemble_is_told_so() {
    let mut client = pair(false);
    let e = client.read_property(599, DEVICE, 76).unwrap_err();
    assert!(
        matches!(e, Error::Request(RequestError::Abort(4))),
        "segmentation-not-supported: {e}"
    );
    // What fits still reads.
    let name = client.read_property(599, DEVICE, 77).unwrap();
    assert!(matches!(
        name.primitive(),
        Some(Primitive::CharacterString(_))
    ));
}
