// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

// Shared by several test binaries, each of which uses a subset.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

//! A simulated gateway on localhost, and a client that has found it.

use bacnet_probe::sim::Sim;
use bacnet_probe::{Client, Device};
use regalium_bacnet_telemetry::{Attribute, AttributeValue, Instrument, Meter};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A client with short timeouts, so a test of silence is quick.
pub(crate) fn client() -> Client {
    Client::bind("127.0.0.1:0".parse().unwrap())
        .unwrap()
        .with_timeout(Duration::from_millis(300))
        .with_retries(1)
}

/// Starts `sim`, finds it, and returns a client bound to it.
pub(crate) fn discover(sim: Sim) -> (Client, Device, SocketAddr) {
    let addr = sim.spawn().unwrap();
    let mut client = client();
    let mut devices = client
        .who_is(addr, false, Duration::from_millis(300))
        .unwrap();
    assert_eq!(
        devices.len(),
        1,
        "the simulated device should answer Who-Is exactly once"
    );
    (client, devices.remove(0), addr)
}

/// One recording: instrument name, labels as text, value.
pub(crate) type Recorded = (String, Vec<(String, String)>, f64);

/// A meter that remembers everything, to assert on labels.
#[derive(Default)]
pub(crate) struct Recorder(pub(crate) Mutex<Vec<Recorded>>);

impl Meter for Recorder {
    fn record(&self, instrument: &Instrument, value: f64, attributes: &[Attribute<'_>]) {
        let labels = attributes
            .iter()
            .map(|a| {
                let v = match a.value {
                    AttributeValue::Str(s) => s.to_owned(),
                    AttributeValue::Int(n) => n.to_string(),
                    AttributeValue::Bool(b) => b.to_string(),
                };
                (a.key.name().to_owned(), v)
            })
            .collect();
        self.0
            .lock()
            .unwrap()
            .push((instrument.name.to_owned(), labels, value));
    }
}

impl Recorder {
    pub(crate) fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Recordings of `name` whose labels include every `(key, value)` given.
    pub(crate) fn find(&self, name: &str, with: &[(&str, &str)]) -> Vec<Recorded> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|(n, labels, _)| {
                n == name
                    && with
                        .iter()
                        .all(|(k, v)| labels.iter().any(|(lk, lv)| lk == k && lv == v))
            })
            .cloned()
            .collect()
    }
}
