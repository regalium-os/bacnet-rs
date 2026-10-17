// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! The driver over MS/TP on a simulated RS-485 bus: two masters find each
//! other from silence, pass the token, and a client discovers and reads a
//! device through it.

use regalium_bacnet::mstp::master::Config as Station;
use regalium_bacnet::mstp::{Line, Mstp};
use regalium_bacnet::obj::Memory;
use regalium_bacnet::primitive::{ObjectIdentifier, Primitive};
use regalium_bacnet::runtime::Config;
use regalium_bacnet::{Client, Server};
use std::io;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// Every octet written, and who wrote it: what a half-duplex pair carries.
#[derive(Default)]
struct Bus {
    octets: Mutex<Vec<(usize, u8)>>,
    written: Condvar,
}

/// One station's tap on the bus. It does not hear its own writes.
struct Tap {
    bus: Arc<Bus>,
    me: usize,
    read: usize,
}

impl Line for Tap {
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
        let octets = self.bus.octets.lock().unwrap();
        let (octets, _) = self
            .bus
            .written
            .wait_timeout_while(octets, timeout, |o| o.len() <= self.read)
            .unwrap();
        let mut n = 0;
        while self.read < octets.len() && n < buf.len() {
            let (who, b) = octets[self.read];
            self.read += 1;
            if who != self.me {
                buf[n] = b;
                n += 1;
            }
        }
        Ok(n)
    }

    fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.bus
            .octets
            .lock()
            .unwrap()
            .extend(data.iter().map(|b| (self.me, *b)));
        self.bus.written.notify_all();
        Ok(())
    }
}

#[test]
fn a_client_finds_and_reads_a_device_over_ms_tp() {
    let bus = Arc::new(Bus::default());
    let tap = |me| Tap {
        bus: bus.clone(),
        me,
        read: 0,
    };
    // Stations 0..=3, so polling for a successor is quick.
    let client_link = Mstp::new(tap(1), Station::new(1).max_master(3));
    let device_link = Mstp::new(tap(2), Station::new(2).max_master(3));

    let ai1 = ObjectIdentifier {
        object_type: 0,
        instance: 1,
    };
    let db = Memory::new(77, "Lift MS/TP", 260)
        .object(ai1, "Car load")
        .with(ai1, 85, &Primitive::Real(42.0));
    let mut server = Server::new(device_link, db);
    std::thread::spawn(move || server.serve(Duration::from_secs(5)).unwrap());

    let mut client = Client::new(client_link, Config::default());
    let found = client
        .who_is(Some((77, 77)), Duration::from_secs(2))
        .unwrap();
    assert_eq!(
        found
            .iter()
            .map(|d| (d.instance, d.address))
            .collect::<Vec<_>>(),
        [(77, 2)]
    );
    assert_eq!(
        client.read_property(77, ai1, 85).unwrap().primitive(),
        Some(Primitive::Real(42.0))
    );
}
