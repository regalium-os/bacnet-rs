// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! `ReadProperty`, `ReadPropertyMultiple`, and the descent from one to the
//! other, against the simulated gateway over real UDP.

mod common;

use bacnet_probe::scan::{self, Options};
use bacnet_probe::service::PropertyRef;
use bacnet_probe::service::rpm::Spec;
use bacnet_probe::sim::{self, Sim};
use bacnet_probe::{Device, ReadError, Value, property};
use std::net::UdpSocket;

#[test]
fn who_is_finds_the_device_and_its_i_am_parameters() {
    let (_, device, _) = common::discover(Sim::lift_gateway());
    assert_eq!(
        (device.instance, device.max_apdu, device.vendor),
        (599, 1476, 999)
    );
    assert_eq!(device.route, None);
}

#[test]
fn the_object_list_reads_element_by_element_and_the_point_reads_live() {
    let (mut client, device, _) = common::discover(Sim::lift_gateway());
    let mut read = |o, p, i| client.read_property(&device, o, p, i).unwrap();
    assert_eq!(
        read(sim::DEVICE, property::OBJECT_LIST, Some(0)),
        [Value::Unsigned(6)]
    );
    assert_eq!(
        read(sim::DEVICE, property::OBJECT_LIST, Some(2)),
        [Value::Object(sim::CAR_LOAD)]
    );
    assert_eq!(
        read(sim::CAR_LOAD, property::PRESENT_VALUE, None),
        [Value::Real(420.0)]
    );
}

#[test]
fn one_read_property_multiple_returns_a_value_or_an_error_per_property() {
    let (mut client, device, _) = common::discover(Sim::lift_gateway());
    let specs = [
        Spec {
            object: sim::CAR_LOAD,
            properties: vec![PropertyRef::whole(85), PropertyRef::whole(9999)],
        },
        Spec {
            object: sim::FAN,
            properties: vec![PropertyRef {
                property: 87,
                index: Some(8),
            }],
        },
    ];
    let results = client.read_property_multiple(&device, &specs).unwrap();
    assert_eq!(results[0].properties[0].1, Ok(vec![Value::Real(420.0)]));
    assert_eq!(results[0].properties[1].1, Err((2, 32)));
    assert_eq!(results[1].properties[0].1, Ok(vec![Value::Real(60.0)]));
}

#[test]
fn a_device_without_read_property_multiple_is_read_singly_after_one_reject() {
    let (mut client, device, _) = common::discover(Sim::lift_gateway().without_rpm());
    assert!(client.supports_rpm(device.instance));
    let report = scan::device(
        &mut client,
        &device,
        &Options {
            detail: false,
            max_objects: 100,
            only: None,
        },
        |_, _| {},
    );
    assert!(
        !client.supports_rpm(device.instance),
        "the reject is remembered"
    );
    assert_eq!(report.object_count, Some(6));
    assert_eq!(
        report.objects.len(),
        5,
        "every object but the device itself"
    );
    let load = report
        .objects
        .iter()
        .find(|o| o.id == sim::CAR_LOAD)
        .unwrap();
    assert_eq!(load.present_value(), Some(&Value::Real(420.0)));
    assert_eq!(load.units(), Some(39));
}

#[test]
fn a_scan_through_read_property_multiple_matches_a_scan_without_it() {
    let opts = Options {
        detail: true,
        max_objects: 100,
        only: None,
    };
    let (mut fast, d1, _) = common::discover(Sim::lift_gateway());
    let (mut slow, d2, _) = common::discover(Sim::lift_gateway().without_rpm());
    let a = scan::device(&mut fast, &d1, &opts, |_, _| {});
    let b = scan::device(&mut slow, &d2, &opts, |_, _| {});
    let props = |r: &scan::DeviceReport| {
        r.objects
            .iter()
            .map(|o| (o.id, o.reads.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(props(&a), props(&b));
    assert!(fast.supports_rpm(599));
}

#[test]
fn an_unknown_property_comes_back_as_a_typed_bacnet_error() {
    let (mut client, device, _) = common::discover(Sim::lift_gateway());
    let err = client
        .read_property(&device, sim::CAR_LOAD, 9999, None)
        .unwrap_err();
    assert!(
        matches!(err, ReadError::Bacnet { class: 2, code: 32 }),
        "got {err:?}"
    );
}

#[test]
fn an_index_into_a_scalar_property_is_refused_as_not_an_array() {
    let (mut client, device, _) = common::discover(Sim::lift_gateway());
    let err = client
        .read_property(&device, sim::CAR_LOAD, property::PRESENT_VALUE, Some(1))
        .unwrap_err();
    assert!(
        matches!(err, ReadError::Bacnet { class: 2, code: 50 }),
        "got {err:?}"
    );
}

#[test]
fn a_silent_device_times_out_after_its_retries() {
    let silent = UdpSocket::bind("127.0.0.1:0").unwrap();
    let device = Device {
        instance: 1,
        addr: silent.local_addr().unwrap(),
        route: None,
        max_apdu: 480,
        vendor: 0,
    };
    let err = common::client()
        .read_property(&device, sim::DEVICE, property::OBJECT_NAME, None)
        .unwrap_err();
    assert!(matches!(err, ReadError::Timeout), "got {err:?}");
}
