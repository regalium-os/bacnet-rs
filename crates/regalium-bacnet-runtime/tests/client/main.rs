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
//! at MAC 0x0a, behind a router at MAC 2. What each answers is in
//! `devices.rs`.

mod alarms;
mod devices;
mod discovery;
mod reads;
mod rebind;
mod services;

use regalium_bacnet_core::datalink::{Datalink, Received, Recipient};
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
    pub(crate) moved: AtomicBool,
}

impl Net {
    /// Puts an NPDU from `from` on the network, as a device would unasked.
    pub(crate) fn push(&self, from: u8, npdu: &[u8]) {
        self.inbox.lock().unwrap().push_back((from, npdu.to_vec()));
    }
}

impl Datalink for Net {
    type Mac = u8;
    type Error = std::io::Error;

    fn send(&self, to: Recipient<u8>, npdu: &[u8]) -> std::io::Result<()> {
        self.sent.lock().unwrap().push((to, npdu.to_vec()));
        if !self.silent.load(Ordering::SeqCst) {
            self.inbox.lock().unwrap().extend(devices::answer(
                self.moved.load(Ordering::SeqCst),
                to,
                npdu,
            ));
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

/// A client on a fresh network, with timers short enough for a test.
pub(crate) fn client() -> Client<Net> {
    let timers = tsm::Config::default()
        .apdu_timeout(Duration::from_millis(30))
        .retries(1);
    Client::new(
        Net::default(),
        Config::default()
            .transactions(timers)
            .rebind(Some(Duration::from_millis(50))),
    )
}
