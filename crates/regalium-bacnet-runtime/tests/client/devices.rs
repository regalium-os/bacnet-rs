// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What the scripted devices answer. Device 599 does everything: reads,
//! `ReadPropertyMultiple`, writes, COV — and after a subscription to
//! analog-input 2, a *confirmed* notification; and a read of analog-input
//! 7 trips that point's high-limit alarm, sent as a confirmed event
//! notification with invoke id 88. Device 10, behind its
//! router, rejects `ReadPropertyMultiple` and refuses COV, as plenty of
//! field controllers do. Device 599 can move to MAC 3, as a controller does
//! when it comes back on a new DHCP lease.

use regalium_bacnet_core::apdu::{Apdu, Pdu};
use regalium_bacnet_core::datalink::Recipient;
use regalium_bacnet_core::npdu::{Body, Npdu};
use regalium_bacnet_core::primitive::ObjectIdentifier;
use regalium_bacnet_core::service::{
    CovNotification, PropertyReference, PropertyResult, PropertyValue, ReadProperty,
    ReadPropertyAck, ReadPropertyMultiple, ReadPropertyMultipleAck, ReadResult, ServiceError,
    SubscribeCov, WhoIs, WriteProperty,
};

/// Answer headers: local from 599, or from 10 by way of the router, which
/// adds the source network (§6.2.2).
const LOCAL: [u8; 2] = [0x01, 0x00];
const ROUTED: [u8; 6] = [0x01, 0x08, 0x00, 0x05, 0x01, 0x0a];
const REAL_21_5: [u8; 5] = [0x44, 0x41, 0xac, 0x00, 0x00];
const REAL_22: [u8; 5] = [0x44, 0x41, 0xb0, 0x00, 0x00];
const FLAGS_CLEAR: [u8; 3] = [0x82, 0x04, 0x00];

/// What the devices answer to one NPDU.
pub(crate) fn answer(moved: bool, to: Recipient<u8>, npdu: &[u8]) -> Vec<(u8, Vec<u8>)> {
    // Device 599 is at MAC 1, or at 3 once it has moved.
    let here = if moved { 3 } else { 1 };
    let n = Npdu::decode(npdu).unwrap();
    let Body::Apdu(apdu) = n.body else {
        return Vec::new();
    };
    let apdu = Apdu::decode(apdu).unwrap();
    let routed = n.destination.is_some_and(|d| d.address.network == 5);
    if let Pdu::UnconfirmedRequest { service: 8, body } = apdu.pdu {
        let who = WhoIs::decode(body).unwrap();
        let mut out = Vec::new();
        if who.asks(599) {
            out.push((here, [&LOCAL[..], &i_am(599)].concat()));
        }
        if who.asks(10) {
            out.push((2, [&ROUTED[..], &i_am(10)].concat()));
        }
        return out;
    }
    let Pdu::ConfirmedRequest {
        invoke_id: id,
        service,
        body,
        ..
    } = apdu.pdu
    else {
        return Vec::new();
    };
    let (mac, head) = match (to, routed) {
        (Recipient::Unicast(2), true) => (2, &ROUTED[..]),
        (Recipient::Unicast(m), false) if m == here => (1, &LOCAL[..]),
        _ => return Vec::new(),
    };
    // `mac` names the device (1 is 599, 2 is 10); replies come from where
    // it is.
    let from = if mac == 1 { here } else { mac };
    let reply = |apdu: Vec<u8>| (from, [head, &apdu].concat());
    match (mac, service) {
        (1, 12) if ReadProperty::decode(body).unwrap().object.instance == 7 => {
            vec![
                reply(alarm()),
                reply(read(id, &ReadProperty::decode(body).unwrap())),
            ]
        }
        (_, 12) => vec![reply(read(id, &ReadProperty::decode(body).unwrap()))],
        (1, 14) => vec![reply(rpm(id, &ReadPropertyMultiple::decode(body).unwrap()))],
        (2, 14) => vec![reply(vec![0x60, id, 9])],
        (_, 15) => {
            let w = WriteProperty::decode(body).unwrap();
            let ok = w.object.instance == 1;
            vec![reply(if ok {
                vec![0x20, id, 15]
            } else {
                vec![0x50, id, 15, 0x91, 0x02, 0x91, 0x28]
            })]
        }
        (1, 5) => {
            let s = SubscribeCov::decode(body).unwrap();
            let mut out = vec![reply(vec![0x20, id, 5])];
            if !s.is_cancellation() {
                out.push(reply(notification(
                    s.process,
                    s.object,
                    s.object.instance == 2,
                )));
            }
            out
        }
        (2, 5) => vec![reply(vec![0x50, id, 5, 0x91, 0x05, 0x91, 0x2d])],
        _ => Vec::new(),
    }
}

fn i_am(instance: u32) -> Vec<u8> {
    let mut out = vec![0x10, 0x00, 0xc4];
    out.extend(((8u32 << 22) | instance).to_be_bytes());
    out.extend([0x22, 0x05, 0xc4, 0x91, 0x03, 0x21, 0x05]);
    out
}

/// Present-value 21.5 for analog-input 1; unknown-object for anything else.
fn read(invoke: u8, r: &ReadProperty) -> Vec<u8> {
    if r.object.object_type == 0 && r.object.instance == 1 {
        let mut out = vec![0x30, invoke, 0x0c];
        ReadPropertyAck {
            read: *r,
            value: &REAL_21_5,
        }
        .encode(&mut out);
        out
    } else {
        vec![0x50, invoke, 0x0c, 0x91, 0x01, 0x91, 0x1f]
    }
}

/// Present-value and status-flags; unknown-property for anything else.
fn rpm(invoke: u8, r: &ReadPropertyMultiple) -> Vec<u8> {
    let objects = r
        .reads
        .iter()
        .map(|a| ReadResult {
            object: a.object,
            results: a
                .properties
                .iter()
                .map(|p| PropertyResult {
                    property: *p,
                    result: match p.property {
                        85 => Ok(&REAL_21_5[..]),
                        111 => Ok(&FLAGS_CLEAR[..]),
                        _ => Err(ServiceError { class: 2, code: 32 }),
                    },
                })
                .collect(),
        })
        .collect();
    let mut out = vec![0x30, invoke, 14];
    ReadPropertyMultipleAck { objects }.encode(&mut out);
    out
}

/// A notification of present-value 22.0: unconfirmed, or confirmed with
/// invoke id 77, which the client must acknowledge.
fn notification(process: u32, object: ObjectIdentifier, confirmed: bool) -> Vec<u8> {
    let n = CovNotification {
        process,
        device: ObjectIdentifier {
            object_type: 8,
            instance: 599,
        },
        object,
        time_remaining: 300,
        values: vec![PropertyValue {
            property: PropertyReference::new(85),
            value: &REAL_22,
            priority: None,
        }],
    };
    let mut out = if confirmed {
        vec![0x00, 0x05, 77, 0x01]
    } else {
        vec![0x10, 0x02]
    };
    n.encode(&mut out);
    out
}

/// A confirmed high-limit alarm on analog-input 7, sent with invoke id 88
/// just before the answer to the read that tripped it.
fn alarm() -> Vec<u8> {
    let mut out = vec![0x00, 0x05, 88, 0x02];
    out.extend([
        0x09, 0x01, 0x1c, 0x02, 0x00, 0x02, 0x57, 0x2c, 0x00, 0x00, 0x00, 0x07,
    ]);
    out.extend([0x3e, 0x19, 0x2a, 0x3f, 0x49, 0x05, 0x59, 0x64, 0x69, 0x05]);
    out.extend([
        0x7d, 0x0a, 0x00, b'H', b'i', b'g', b'h', b' ', b'l', b'o', b'a', b'd',
    ]);
    out.extend([0x89, 0x00, 0x99, 0x01, 0xa9, 0x00, 0xb9, 0x03]);
    out
}

/// The alarm as a whole NPDU, for a test to put on the network itself.
pub(crate) fn alarm_npdu() -> Vec<u8> {
    [&LOCAL[..], &alarm()].concat()
}
