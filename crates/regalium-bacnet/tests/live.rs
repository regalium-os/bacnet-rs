// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! The whole driver over a real UDP socket: a device on loopback, found by
//! Who-Is and read by `ReadProperty`, every octet through the codecs, the
//! transaction state machine, BACnet/IP and the client.

use regalium_bacnet::Client;
use regalium_bacnet::apdu::{Apdu, Pdu};
use regalium_bacnet::ip::BacnetIp;
use regalium_bacnet::ip::bvll::{Bvll, Function};
use regalium_bacnet::npdu::{Body, Npdu};
use regalium_bacnet::primitive::{ObjectIdentifier, Primitive};
use regalium_bacnet::runtime::Config;
use regalium_bacnet::service::{ReadProperty, ReadPropertyAck};
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::time::Duration;

/// A device 599 that answers Who-Is and reads of analog-input 1, on its own
/// thread until five seconds of silence — longer than a discovery listens.
fn device() -> SocketAddrV4 {
    let s = UdpSocket::bind("127.0.0.1:0").unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let std::net::SocketAddr::V4(addr) = s.local_addr().unwrap() else {
        panic!()
    };
    std::thread::spawn(move || {
        let mut buf = [0u8; 1600];
        while let Ok((n, from)) = s.recv_from(&mut buf) {
            let bvll = Bvll::decode(&buf[..n]).unwrap();
            let npdu = Npdu::decode(bvll.npdu().unwrap()).unwrap();
            let Body::Apdu(a) = npdu.body else { continue };
            let reply = match Apdu::decode(a).unwrap().pdu {
                Pdu::UnconfirmedRequest { service: 8, .. } => {
                    vec![
                        0x01, 0x00, 0x10, 0x00, 0xc4, 0x02, 0x00, 0x02, 0x57, 0x22, 0x05, 0xc4,
                        0x91, 0x03, 0x21, 0x05,
                    ]
                }
                Pdu::ConfirmedRequest {
                    invoke_id,
                    service: 12,
                    body,
                    ..
                } => {
                    let mut r = vec![0x01, 0x00, 0x30, invoke_id, 0x0c];
                    let read = ReadProperty::decode(body).unwrap();
                    ReadPropertyAck {
                        read,
                        value: &[0x44, 0x41, 0xac, 0x00, 0x00],
                    }
                    .encode(&mut r);
                    r
                }
                _ => continue,
            };
            s.send_to(&Bvll::to_vec(Function::OriginalUnicast(&reply)), from)
                .unwrap();
        }
    });
    addr
}

#[test]
fn a_device_on_udp_is_found_and_read_through_the_whole_driver() {
    let dev = device();
    let link = BacnetIp::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap();
    let mut client = Client::new(link, Config::default());
    let found = client
        .who_is_at(dev, Some((599, 599)), Duration::from_secs(2))
        .unwrap();
    assert_eq!(
        found
            .iter()
            .map(|d| (d.instance, d.address))
            .collect::<Vec<_>>(),
        [(599, dev)]
    );
    let ai1 = ObjectIdentifier {
        object_type: 0,
        instance: 1,
    };
    let reading = client.read_property(599, ai1, 85).unwrap();
    assert_eq!(reading.primitive(), Some(Primitive::Real(21.5)));
    let p = reading.property().unwrap();
    assert_eq!(
        p.name,
        "devices/599/objects/analog-input-1/properties/present-value"
    );
    assert_eq!(p.value.unwrap().encoded, [0x44, 0x41, 0xac, 0x00, 0x00]);
}
