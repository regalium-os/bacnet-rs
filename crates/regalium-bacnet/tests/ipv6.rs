// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! The driver over BACnet/IPv6: a device served on one link, a client on
//! another, discovery resolving virtual MACs from the I-Am and the read
//! that follows going unicast to the address learnt.

use regalium_bacnet::ipv6::BacnetIpv6;
use regalium_bacnet::ipv6::bvll::Vmac;
use regalium_bacnet::obj::Memory;
use regalium_bacnet::primitive::{ObjectIdentifier, Primitive};
use regalium_bacnet::runtime::Config;
use regalium_bacnet::{Client, Server};
use std::net::{Ipv6Addr, SocketAddrV6};
use std::time::Duration;

#[test]
fn a_client_finds_and_reads_a_device_over_bacnet_ipv6() {
    let any = SocketAddrV6::new(Ipv6Addr::LOCALHOST, 0, 0, 0);
    let client_link = BacnetIpv6::bind(any, Vmac::from_instance(1000)).unwrap();
    let device_link = BacnetIpv6::bind(any, Vmac::from_instance(599)).unwrap();
    // On loopback each one's multicast group is the other.
    let (client_at, device_at) = (
        client_link.local_addr().unwrap(),
        device_link.local_addr().unwrap(),
    );
    let client_link = client_link.group(device_at);
    let device_link = device_link.group(client_at);

    let ai1 = ObjectIdentifier {
        object_type: 0,
        instance: 1,
    };
    let db = Memory::new(599, "IPv6 gateway", 260)
        .object(ai1, "Load")
        .with(ai1, 85, &Primitive::Real(7.5));
    let mut server = Server::new(device_link, db);
    std::thread::spawn(move || server.serve(Duration::from_secs(3)).unwrap());

    let mut client = Client::new(client_link, Config::default());
    let found = client
        .who_is(Some((599, 599)), Duration::from_millis(300))
        .unwrap();
    assert_eq!(
        found
            .iter()
            .map(|d| (d.instance, d.address))
            .collect::<Vec<_>>(),
        [(599, Vmac([0, 2, 0x57]))]
    );
    assert_eq!(
        client.read_property(599, ai1, 85).unwrap().primitive(),
        Some(Primitive::Real(7.5))
    );
}
