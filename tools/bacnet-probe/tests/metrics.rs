// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

//! What reaches the meter: request counters by service and outcome, and
//! point gauges labelled by which point they are.

mod common;

use bacnet_probe::metrics;
use bacnet_probe::scan::{self, Options};
use bacnet_probe::sim::{self, Sim};
use std::sync::Arc;

#[test]
fn every_request_is_counted_by_service_outcome_and_device() {
    let (client, device, _) = common::discover(Sim::lift_gateway());
    let recorder = common::Recorder::shared();
    let mut client = client.with_meter(Arc::clone(&recorder) as _);
    client
        .read_property(&device, sim::CAR_LOAD, 85, None)
        .unwrap();
    let _ = client.read_property(&device, sim::CAR_LOAD, 9999, None);
    let ok = [
        ("bacnet.service", "readProperty"),
        ("bacnet.outcome", "ok"),
        ("bacnet.device.instance", "599"),
    ];
    assert_eq!(recorder.find("bacnet.client.requests", &ok).len(), 1);
    assert_eq!(
        recorder
            .find("bacnet.client.requests", &[("bacnet.outcome", "error")])
            .len(),
        1
    );
    assert_eq!(
        recorder
            .find(
                "bacnet.client.request.duration",
                &[("bacnet.service", "readProperty")]
            )
            .len(),
        2
    );
    assert_ne!(
        recorder.find("bacnet.client.packets", &[("bacnet.direction", "sent")]),
        [] as [(
            std::string::String,
            std::vec::Vec<(std::string::String, std::string::String)>,
            f64
        ); 0]
    );
}

#[test]
fn a_scanned_point_is_exported_with_every_identifying_label() {
    let (mut client, device, _) = common::discover(Sim::lift_gateway());
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
    let recorder = common::Recorder::default();
    // The scan read the device's identity, and the client remembers it.
    let identity = client.identity(599).cloned().expect("learnt from the scan");
    for o in &report.objects {
        metrics::record_object(&recorder, &identity, o);
    }
    let load = recorder.find(
        "bacnet.point.value",
        &[
            ("bacnet.device.instance", "599"),
            ("bacnet.object.type", "analog-input"),
            ("bacnet.object.instance", "1"),
            ("bacnet.object.name", "Car 1 load"),
            ("bacnet.object.kind", "analog"),
            ("bacnet.units", "kilograms"),
            // Who the device is, on every point: a filter, not a join.
            ("bacnet.device.name", "Lift gateway (simulated)"),
            ("bacnet.vendor.name", "RegaliumOS simulator"),
            ("bacnet.vendor.id", "999"),
            ("bacnet.device.model", "probe-sim"),
            ("bacnet.device.firmware", "1.0.0"),
            ("bacnet.device.software", "sim 2026.10"),
            ("bacnet.device.protocol_revision", "22"),
            ("bacnet.device.location", "Machine room, level 12"),
        ],
    );
    assert_eq!(load.len(), 1);
    assert!((load[0].2 - 420.0).abs() < f64::EPSILON);
    let alarm = recorder.find(
        "bacnet.point.status",
        &[
            ("bacnet.object.instance", "2"),
            ("bacnet.status.flag", "in-alarm"),
        ],
    );
    assert_eq!(
        alarm.first().map(|r| r.2),
        Some(1.0),
        "the machine room is in alarm"
    );
    let door = recorder.find(
        "bacnet.point.value",
        &[("bacnet.object.type", "binary-input")],
    );
    assert_eq!(
        door.first().map(|r| r.2),
        Some(0.0),
        "binary points export 0/1"
    );
}

#[test]
fn a_value_is_never_a_label() {
    let (mut client, device, _) = common::discover(Sim::lift_gateway());
    let report = scan::device(
        &mut client,
        &device,
        &Options {
            detail: true,
            max_objects: 100,
            only: None,
        },
        |_, _| {},
    );
    let recorder = common::Recorder::default();
    for o in &report.objects {
        metrics::record_object(&recorder, &client.identity_or(599, ""), o);
    }
    let all = recorder.0.lock().unwrap();
    for (_, labels, _) in all.iter() {
        assert!(
            labels
                .iter()
                .all(|(_, v)| v != "420" && v != "420.00" && !v.contains("Normal")),
            "{labels:?}"
        );
    }
}
