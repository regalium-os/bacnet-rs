// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! The driver talking to itself over UDP: a local device served from a
//! database on one socket, a client on another, every octet through both
//! sides of the codecs, the state machine and BACnet/IP.

use regalium_bacnet::datalink::Datalink;
use regalium_bacnet::ip::BacnetIp;
use regalium_bacnet::obj::Memory;
use regalium_bacnet::primitive::{ObjectIdentifier, Primitive};
use regalium_bacnet::runtime::{Config, Error, RequestError, Source, Watch};
use regalium_bacnet::service::{PropertyReference, ReadAccess, ServiceError};
use regalium_bacnet::{Client, Server};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

const AI1: ObjectIdentifier = ObjectIdentifier {
    object_type: 0,
    instance: 1,
};
const AO1: ObjectIdentifier = ObjectIdentifier {
    object_type: 1,
    instance: 1,
};

#[test]
fn a_client_finds_reads_writes_and_watches_a_device_the_driver_serves() {
    let loopback = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0);
    let client_link = BacnetIp::bind(loopback).unwrap();
    // The device's "subnet broadcast" is the client: on loopback there is
    // no subnet to broadcast to.
    let device_link = BacnetIp::bind(loopback)
        .unwrap()
        .broadcast_address(client_link.local_addr());
    let device_addr = device_link.local_addr();
    let db = Memory::new(599, "Lift gateway", 260)
        .object(AI1, "Car 1 load")
        .with(AI1, 85, &Primitive::Real(420.0))
        .object(AO1, "Fan speed")
        .commandable(AO1, &Primitive::Real(0.0));
    let mut server = Server::new(device_link, db);
    let serving = std::thread::spawn(move || {
        server.announce().unwrap();
        server.serve(Duration::from_secs(4)).unwrap();
        server
    });

    let mut client = Client::new(client_link, Config::default());
    let found = client
        .who_is_at(device_addr, None, Duration::from_millis(300))
        .unwrap();
    assert_eq!(
        found
            .iter()
            .map(|d| (d.instance, d.vendor_id))
            .collect::<Vec<_>>(),
        [(599, 260)]
    );
    assert_eq!(
        client.read_property(599, AI1, 85).unwrap().primitive(),
        Some(Primitive::Real(420.0))
    );

    client
        .write_property(599, AO1, 85, &Primitive::Real(42.5), Some(8))
        .unwrap();
    assert_eq!(
        client.read_property(599, AO1, 85).unwrap().primitive(),
        Some(Primitive::Real(42.5))
    );
    let refused = client
        .write_property(599, AI1, 85, &Primitive::Real(1.0), None)
        .unwrap_err();
    assert!(matches!(
        refused,
        Error::Request(RequestError::Error(ServiceError { class: 2, code: 40 }))
    ));

    let all = [ReadAccess {
        object: AI1,
        properties: vec![PropertyReference::new(8)],
    }];
    let props: Vec<u32> = client
        .read_multiple(599, &all)
        .unwrap()
        .iter()
        .map(|r| r.as_ref().unwrap().read.property)
        .collect();
    assert_eq!(props, [75, 77, 79, 85]);

    // The device does not serve COV: the watch says so by polling.
    client
        .watch(Watch::new(599, AI1).every(Duration::from_millis(100)))
        .unwrap();
    client.listen(Duration::from_millis(250)).unwrap();
    let updates: Vec<_> = client.updates().collect();
    assert!(
        !updates.is_empty() && updates.iter().all(|u| u.source == Source::Poll),
        "{updates:?}"
    );

    let server = serving.join().unwrap();
    assert_eq!(server.link().max_apdu(), 1476);
}
