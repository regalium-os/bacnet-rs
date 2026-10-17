// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! The client against an in-memory network: a datalink that hands every
//! request to scripted devices and queues their answers. The client cannot
//! tell it from a socket, which is the point of the trait.
//!
//! Device 599 is on the local network at MAC 1. Device 10 is on network 5
//! at MAC 0x0a, behind a router at MAC 2.

mod discovery;
mod reads;

use regalium_bacnet_core::apdu::{Apdu, Pdu};
use regalium_bacnet_core::datalink::{Datalink, Received, Recipient};
use regalium_bacnet_core::npdu::{Body, Npdu};
use regalium_bacnet_core::service::{ReadProperty, ReadPropertyAck, WhoIs};
use regalium_bacnet_core::tsm;
use regalium_bacnet_runtime::{Client, Config};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// The network: what was sent, what waits to be received, and whether the
/// devices are answering.
#[derive(Default)]
pub(crate) struct Net {
    pub(crate) sent: Mutex<Vec<(Recipient<u8>, Vec<u8>)>>,
    inbox: Mutex<VecDeque<(u8, Vec<u8>)>>,
    pub(crate) silent: AtomicBool,
}

impl Datalink for Net {
    type Mac = u8;
    type Error = std::io::Error;

    fn send(&self, to: Recipient<u8>, npdu: &[u8]) -> std::io::Result<()> {
        self.sent.lock().unwrap().push((to, npdu.to_vec()));
        if !self.silent.load(Ordering::SeqCst) {
            self.inbox.lock().unwrap().extend(answer(to, npdu));
        }
        Ok(())
    }

    fn receive(&self, buf: &mut [u8], timeout: Duration) -> std::io::Result<Option<Received<u8>>> {
        let Some((from, npdu)) = self.inbox.lock().unwrap().pop_front() else {
            std::thread::sleep(timeout);
            return Ok(None);
        };
        buf[..npdu.len()].copy_from_slice(&npdu);
        Ok(Some(Received {
            from,
            len: npdu.len(),
            broadcast: false,
        }))
    }

    fn max_apdu(&self) -> u16 {
        1476
    }
}

/// Header octets of an answer: local from device 599, or from device 10 by
/// way of the router, which adds the source network (§6.2.2).
const LOCAL: [u8; 2] = [0x01, 0x00];
const ROUTED: [u8; 6] = [0x01, 0x08, 0x00, 0x05, 0x01, 0x0a];

/// What the devices answer to one NPDU.
fn answer(to: Recipient<u8>, npdu: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let n = Npdu::decode(npdu).unwrap();
    let Body::Apdu(apdu) = n.body else {
        return Vec::new();
    };
    let apdu = Apdu::decode(apdu).unwrap();
    let routed = n.destination.is_some_and(|d| d.address.network == 5);
    match apdu.pdu {
        Pdu::UnconfirmedRequest { service: 8, body } => {
            let who = WhoIs::decode(body).unwrap();
            let mut out = Vec::new();
            if who.asks(599) {
                out.push((1, [&LOCAL[..], &i_am(599)].concat()));
            }
            if who.asks(10) {
                out.push((2, [&ROUTED[..], &i_am(10)].concat()));
            }
            out
        }
        Pdu::ConfirmedRequest {
            invoke_id,
            service: 12,
            body,
            ..
        } => {
            let (mac, head) = match (to, routed) {
                (Recipient::Unicast(2), true) => (2, &ROUTED[..]),
                (Recipient::Unicast(1), false) => (1, &LOCAL[..]),
                _ => return Vec::new(),
            };
            vec![(
                mac,
                [head, &read(invoke_id, &ReadProperty::decode(body).unwrap())].concat(),
            )]
        }
        _ => Vec::new(),
    }
}

/// An I-Am: the Device object's identifier (type 8 in its top ten bits),
/// 1476 octets, no segmentation, vendor 5.
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
            value: &[0x44, 0x41, 0xac, 0x00, 0x00],
        }
        .encode(&mut out);
        out
    } else {
        vec![0x50, invoke, 0x0c, 0x91, 0x01, 0x91, 0x1f]
    }
}

/// A client on a fresh network, with timers short enough for a test.
pub(crate) fn client() -> Client<Net> {
    let timers = tsm::Config::default()
        .apdu_timeout(Duration::from_millis(30))
        .retries(1);
    Client::new(Net::default(), Config::default().transactions(timers))
}
