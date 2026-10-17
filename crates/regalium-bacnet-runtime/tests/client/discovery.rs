// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Who-Is out, I-Ams in: local devices and routed ones bound alike, each
//! discovery reported once.

use super::client;
use regalium_bacnet_core::datalink::Recipient;
use regalium_bacnet_runtime::{Event, Route};
use std::time::{Duration, Instant};

#[test]
fn a_who_is_binds_local_devices_and_devices_behind_a_router() {
    let mut c = client();
    let found = c.who_is(None, Duration::from_millis(100)).unwrap();
    assert_eq!(
        found.iter().map(|d| d.instance).collect::<Vec<_>>(),
        [10, 599]
    );
    let local = c.device(599).unwrap();
    assert_eq!((local.address, &local.route), (1, &None));
    let routed = c.device(10).unwrap();
    assert_eq!(
        (routed.address, &routed.route),
        (
            2,
            &Some(Route {
                network: 5,
                mac: vec![0x0a]
            })
        )
    );
    assert_eq!(
        c.events().collect::<Vec<_>>(),
        [Event::Discovered(599), Event::Discovered(10)]
    );
    // The Who-Is went to every network: DNET X'FFFF', hop count 255.
    let sent = c.link().sent.lock().unwrap().clone();
    assert_eq!(
        sent,
        [(
            Recipient::LocalBroadcast,
            vec![0x01, 0x20, 0xff, 0xff, 0x00, 0xff, 0x10, 0x08]
        )]
    );
}

#[test]
fn a_second_discovery_reports_nothing_new_and_a_range_asks_fewer() {
    let mut c = client();
    c.who_is(None, Duration::from_millis(50)).unwrap();
    let _ = c.events().count();
    let found = c
        .who_is(Some((500, 700)), Duration::from_millis(50))
        .unwrap();
    assert_eq!(found.iter().map(|d| d.instance).collect::<Vec<_>>(), [599]);
    assert_eq!(c.events().count(), 0);
}

#[test]
fn binding_one_device_returns_as_soon_as_it_answers() {
    let mut c = client();
    let start = Instant::now();
    let d = c.bind_device(10, Duration::from_secs(5)).unwrap().unwrap();
    assert_eq!(d.instance, 10);
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "did not wait out the timeout"
    );
    assert_eq!(c.bind_device(77, Duration::from_millis(50)).unwrap(), None);
}
