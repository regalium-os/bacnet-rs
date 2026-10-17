// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Talking to a device: Who-Is answered by I-Am, junk on the port ignored,
//! and a `ReadProperty` from request to decoded value.

use super::{link, octets, peer};
use regalium_bacnet_core::apdu::Apdu;
use regalium_bacnet_core::datalink::{Datalink, Recipient};
use regalium_bacnet_core::npdu::{Body, Npdu};
use regalium_bacnet_core::primitive::{ObjectIdentifier, Primitive};
use regalium_bacnet_core::service::{ReadProperty, ReadPropertyAck, Service, confirmed};
use regalium_bacnet_core::tsm::{Config, Remote, Reply, Request, Step, Tsm};
use regalium_bacnet_ip::bvll::{Bvll, Function};
use std::time::{Duration, Instant};

const I_AM: &str = "81 0a 00 15 01 00 10 00 c4 02 00 02 57 22 05 c4 91 03 22 01 04";

#[test]
fn a_who_is_is_answered_and_junk_on_the_port_is_never_handed_up() {
    let (dev, seen) = peer(|_, from, s| {
        for junk in ["45 00 00 1c", "81 00 00 06 00 00", "81 0a 00 05"] {
            s.send_to(&octets(junk), from).unwrap();
        }
        s.send_to(&octets(I_AM), from).unwrap();
    });
    let l = link();
    l.send(Recipient::Unicast(dev), &octets("01 00 10 08"))
        .unwrap();
    assert_eq!(
        seen.recv_timeout(Duration::from_secs(2)).unwrap(),
        octets("81 0a 00 08 01 00 10 08")
    );
    let mut buf = [0u8; 1500];
    let got = l
        .receive(&mut buf, Duration::from_secs(2))
        .unwrap()
        .unwrap();
    assert_eq!((got.from, got.broadcast), (dev, false));
    let npdu = Npdu::decode(&buf[..got.len]).unwrap();
    let Body::Apdu(apdu) = npdu.body else {
        panic!("not an APDU")
    };
    let Service::IAm(i) = Service::decode(&Apdu::decode(apdu).unwrap()).unwrap() else {
        panic!("not I-Am")
    };
    assert_eq!((i.device.instance, i.vendor_id), (599, 260));
}

#[test]
fn a_read_property_goes_through_the_tsm_and_the_link_and_comes_back_as_its_value() {
    let (dev, _) = peer(|d, from, s| {
        let Ok(b) = Bvll::decode(d) else { return };
        let Some(npdu) = b.npdu() else { return };
        // A confirmed request: answer present-value 21.5 under its invoke id.
        if let [0x01, 0x04, _, _, invoke, 0x0c, ..] = npdu {
            let mut reply = vec![0x01, 0x00, 0x30, *invoke, 0x0c];
            reply.extend(octets("0c 00 00 00 01 19 55 3e 44 41 ac 00 00 3f"));
            s.send_to(&Bvll::to_vec(Function::OriginalUnicast(&reply)), from)
                .unwrap();
        }
    });
    let l = link();
    let origin = Instant::now();
    let mut tsm = Tsm::new(Config::default());
    let mut body = Vec::new();
    let ai1 = ObjectIdentifier {
        object_type: 0,
        instance: 1,
    };
    ReadProperty {
        object: ai1,
        property: 85,
        index: None,
    }
    .encode(&mut body);
    let remote = Remote {
        max_apdu: l.max_apdu(),
        ..Remote::default()
    };
    tsm.request(
        dev,
        Request::new(confirmed::READ_PROPERTY, body).remote(remote),
        origin.elapsed(),
    )
    .unwrap();
    let mut buf = [0u8; 1500];
    let reply = loop {
        match tsm.poll(origin.elapsed()) {
            Step::Transmit { peer, apdu, .. } => {
                let mut npdu = vec![0x01, 0x04];
                npdu.extend(apdu);
                l.send(Recipient::Unicast(peer), &npdu).unwrap();
            }
            Step::Idle { wake_at } => {
                let wait = wake_at.unwrap().saturating_sub(origin.elapsed());
                if let Some(got) = l.receive(&mut buf, wait).unwrap() {
                    let npdu = Npdu::decode(&buf[..got.len]).unwrap();
                    let Body::Apdu(apdu) = npdu.body else {
                        continue;
                    };
                    tsm.receive(got.from, &Apdu::decode(apdu).unwrap(), origin.elapsed());
                }
            }
            Step::Complete { reply, .. } => break reply,
            Step::Abandon { reason, .. } => panic!("abandoned: {reason}"),
        }
    };
    let Reply::Complex(result) = reply else {
        panic!("not a Complex-ACK")
    };
    let ack = ReadPropertyAck::decode(&result).unwrap();
    assert_eq!(
        (ack.read.object, ack.primitive()),
        (ai1, Some(Primitive::Real(21.5)))
    );
}
