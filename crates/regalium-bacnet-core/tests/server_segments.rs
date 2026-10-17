// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! The responder sending a segmented answer, on a fake clock: the first
//! segment alone, windows as the requester's acknowledgements allow, a lost
//! window sent again, and an Abort that ends it.

use regalium_bacnet_core::apdu::{Apdu, Pdu};
use regalium_bacnet_core::obj::Memory;
use regalium_bacnet_core::primitive::ObjectIdentifier;
use regalium_bacnet_core::server::{Responder, Response};
use regalium_bacnet_core::tsm::Config;
use std::time::Duration;

/// A device of thirty objects: its object list is 155 octets.
fn db() -> Memory {
    let mut db = Memory::new(599, "Big", 1);
    for i in 1..=30 {
        db = db.object(
            ObjectIdentifier {
                object_type: 2,
                instance: i,
            },
            "v",
        );
    }
    db
}

/// A read of the object list, accepting segments, of 50-octet APDUs.
const READ: [u8; 11] = [
    0x02, 0x70, 0x07, 0x0c, 0x0c, 0x02, 0x00, 0x02, 0x57, 0x19, 0x4c,
];

fn sequences(out: &[Response]) -> Vec<u8> {
    out.iter()
        .map(|r| {
            let Response::Reply(b) = r else {
                panic!("a broadcast")
            };
            match Apdu::decode(b).unwrap().pdu {
                Pdu::ComplexAck {
                    segment: Some(s), ..
                } => s.sequence,
                other => panic!("{other:?}"),
            }
        })
        .collect()
}

fn ack(seq: u8, window: u8, nak: bool) -> Vec<u8> {
    vec![if nak { 0x42 } else { 0x40 }, 0x07, seq, window]
}

fn at(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[test]
fn segments_go_out_window_by_window_and_a_lost_one_is_sent_again() {
    let mut db = db();
    let mut r = Responder::new(Config::default().segment_timeout(at(100)).retries(1));
    let mut handle =
        |apdu: &[u8], ms| r.handle(&"client", &Apdu::decode(apdu).unwrap(), &mut db, at(ms));
    // 155 octets in segments of 45: four. The first goes alone.
    assert_eq!(sequences(&handle(&READ, 0)), [0]);
    assert_eq!(sequences(&handle(&ack(0, 2, false), 10)), [1, 2]);
    // Segment 1 was lost: a negative acknowledgement of 0 asks again.
    assert_eq!(sequences(&handle(&ack(0, 2, true), 20)), [1, 2]);
    assert_eq!(sequences(&handle(&ack(2, 2, false), 30)), [3]);
    // The last acknowledged: nothing left to send, nothing to time out.
    assert_eq!(handle(&ack(3, 2, false), 40).len(), 0);
    assert_eq!(r.deadline(), None);
}

#[test]
fn a_silent_requester_gets_the_window_again_until_the_retries_are_spent() {
    let mut db = db();
    let mut r = Responder::new(Config::default().segment_timeout(at(100)).retries(1));
    r.handle(&"client", &Apdu::decode(&READ).unwrap(), &mut db, at(0));
    assert_eq!(r.poll(at(99)).len(), 0);
    let again: Vec<_> = r.poll(at(100)).into_iter().map(|(_, resp)| resp).collect();
    assert_eq!(sequences(&again), [0]);
    assert_eq!(
        r.poll(at(200)).len(),
        0,
        "retries spent: given up, silently"
    );
    assert_eq!(r.deadline(), None);
}

#[test]
fn an_abort_from_the_requester_ends_the_answer() {
    let mut db = db();
    let mut r = Responder::new(Config::default());
    r.handle(&"client", &Apdu::decode(&READ).unwrap(), &mut db, at(0));
    assert!(r.deadline().is_some());
    r.handle(
        &"client",
        &Apdu::decode(&[0x70, 0x07, 0x04]).unwrap(),
        &mut db,
        at(1),
    );
    assert_eq!(r.deadline(), None);
}
