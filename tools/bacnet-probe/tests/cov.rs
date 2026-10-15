// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

//! COV subscriptions: the initial notification, change notifications from a
//! live device, cancellation, and a device that cannot subscribe at all.

mod common;

use bacnet_probe::sim::{self, Sim};
use bacnet_probe::{ReadError, Value};
use std::time::{Duration, Instant};

#[test]
fn a_subscription_delivers_its_initial_notification_with_value_and_flags() {
    let (mut client, device, _) = common::discover(Sim::lift_gateway());
    let process = client.subscribe_cov(&device, sim::CAR_LOAD, 60).unwrap();
    let got = client.notifications(Duration::from_secs(1)).unwrap();
    assert_eq!(got.len(), 1);
    let n = &got[0];
    assert_eq!(
        (n.process, n.device, n.object),
        (process, sim::DEVICE, sim::CAR_LOAD)
    );
    assert_eq!(n.values[0].1, [Value::Real(420.0)]);
    assert_eq!(n.values[1].0.property, 111);
    assert!(n.time_remaining <= 60);
}

#[test]
fn a_live_device_notifies_each_change_without_being_polled() {
    let (mut client, device, _) = common::discover(Sim::lift_gateway().live());
    client.subscribe_cov(&device, sim::CAR_LOAD, 60).unwrap();
    let mut seen = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(4);
    while seen.len() < 3 && Instant::now() < deadline {
        seen.extend(client.notifications(Duration::from_millis(500)).unwrap());
    }
    assert!(
        seen.len() >= 3,
        "initial plus at least two changes; got {}",
        seen.len()
    );
    let values: Vec<&Value> = seen.iter().map(|n| &n.values[0].1[0]).collect();
    assert!(
        values.windows(2).all(|w| w[0] != w[1]),
        "every notification is a change: {values:?}"
    );
}

#[test]
fn a_cancelled_subscription_stops_notifying() {
    let (mut client, device, _) = common::discover(Sim::lift_gateway().live());
    let process = client.subscribe_cov(&device, sim::CAR_LOAD, 60).unwrap();
    client.cancel_cov(&device, sim::CAR_LOAD, process).unwrap();
    let _ = client.notifications(Duration::from_millis(100)).unwrap(); // The initial one.
    assert_eq!(
        client.notifications(Duration::from_millis(1600)).unwrap(),
        [] as [bacnet_probe::service::cov::Notification; 0]
    );
}

#[test]
fn a_device_without_cov_says_so_and_the_caller_can_poll_instead() {
    let (mut client, device, _) = common::discover(Sim::lift_gateway().without_cov());
    let err = client
        .subscribe_cov(&device, sim::CAR_LOAD, 60)
        .unwrap_err();
    assert!(matches!(err, ReadError::Reject(9)), "got {err:?}");
    assert!(err.unsupported());
}

#[test]
fn a_notification_that_arrives_during_a_read_is_kept_not_dropped() {
    let (mut client, device, _) = common::discover(Sim::lift_gateway());
    client.subscribe_cov(&device, sim::DOOR, 60).unwrap();
    // The initial notification is already on the wire; a read consumes the
    // socket first, and must queue it rather than discard it.
    client.read_property(&device, sim::FAN, 85, None).unwrap();
    let got = client.notifications(Duration::ZERO).unwrap();
    assert_eq!(got.len(), 1, "queued during the read");
    assert_eq!(got[0].object, sim::DOOR);
}
