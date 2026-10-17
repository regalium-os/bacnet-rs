// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A foreign device through a BBMD: it registers inside `receive`, hears
//! broadcasts the BBMD forwards as coming from their origin, and sends its
//! own broadcasts to the BBMD to distribute.

use super::{link, octets, peer};
use regalium_bacnet_core::datalink::{Datalink, Recipient};
use regalium_bacnet_ip::bvll::{Bvll, Function};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

#[test]
fn a_foreign_device_registers_hears_forwarded_broadcasts_and_distributes_its_own() {
    let origin = SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 5), 47808);
    let (bbmd, seen) = peer(move |d, from, s| {
        if let Ok(Bvll {
            function: Function::RegisterForeignDevice { .. },
            ..
        }) = Bvll::decode(d)
        {
            s.send_to(&octets("81 00 00 06 00 00"), from).unwrap();
            let i_am = octets("01 00 10 00 c4 02 00 00 0a 22 01 e0 91 03 21 05");
            s.send_to(
                &Bvll::to_vec(Function::Forwarded {
                    origin,
                    npdu: &i_am,
                }),
                from,
            )
            .unwrap();
        }
    });
    let l = link().foreign(bbmd, 60);
    assert!(!l.is_registered());
    let mut buf = [0u8; 1500];
    let got = l
        .receive(&mut buf, Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert_eq!(
        seen.recv_timeout(Duration::from_secs(1)).unwrap(),
        octets("81 05 00 06 00 3c")
    );
    assert_eq!(
        (got.from, got.broadcast),
        (origin, true),
        "an answer goes to the origin, not the BBMD"
    );
    assert!(l.is_registered());

    l.send(
        Recipient::LocalBroadcast,
        &octets("01 20 ff ff 00 ff 10 08"),
    )
    .unwrap();
    assert_eq!(
        seen.recv_timeout(Duration::from_secs(2)).unwrap(),
        octets("81 09 00 0c 01 20 ff ff 00 ff 10 08"),
        "a foreign device's broadcast goes to its BBMD to distribute"
    );
}

#[test]
fn a_bbmd_that_never_answers_is_asked_again_and_the_link_is_not_registered() {
    let (bbmd, seen) = peer(|_, _, _| {});
    let l = link().foreign(bbmd, 60);
    assert!(
        l.receive(&mut [0; 1500], Duration::from_millis(2300))
            .unwrap()
            .is_none()
    );
    let asked: Vec<_> = seen.try_iter().collect();
    assert_eq!(asked.len(), 2, "at once, then after the 2 s retry");
    assert!(!l.is_registered());
}

#[test]
fn a_bbmd_link_relays_a_broadcast_it_hears_to_its_peers_and_delivers_it_too() {
    use regalium_bacnet_ip::bbmd::Bbmd;
    use regalium_bacnet_ip::bvll::BdtEntry;
    let (peer_bbmd, seen) = peer(|_, _, _| {});
    let l = link();
    let me = l.local_addr();
    let table = vec![
        BdtEntry {
            address: me,
            mask: Ipv4Addr::BROADCAST,
        },
        BdtEntry {
            address: peer_bbmd,
            mask: Ipv4Addr::BROADCAST,
        },
    ];
    let l = l.bbmd(Bbmd::new(me, table));
    // A station on this subnet broadcasts a Who-Is; it reaches the link.
    let station = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    station
        .send_to(&octets("81 0b 00 0c 01 20 ff ff 00 ff 10 08"), me)
        .unwrap();
    let mut buf = [0u8; 1500];
    let got = l
        .receive(&mut buf, Duration::from_secs(2))
        .unwrap()
        .unwrap();
    let std::net::SocketAddr::V4(from) = station.local_addr().unwrap() else {
        panic!()
    };
    assert_eq!((got.from, got.broadcast), (from, true));
    let mut forwarded = octets("81 04 00 12");
    forwarded.extend(from.ip().octets());
    forwarded.extend(from.port().to_be_bytes());
    forwarded.extend(octets("01 20 ff ff 00 ff 10 08"));
    assert_eq!(
        seen.recv_timeout(Duration::from_secs(2)).unwrap(),
        forwarded
    );
}
