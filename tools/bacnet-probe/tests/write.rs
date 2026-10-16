// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

//! Writing properties, against the simulated gateway over real UDP: what a
//! device accepts, and the error code for what it refuses. Creating and
//! deleting objects is in `objects.rs`.

mod common;

use bacnet_probe::service::PropertyRef;
use bacnet_probe::service::write::PropertyValue;
use bacnet_probe::sim::{self, Sim};
use bacnet_probe::{Client, Device, ObjectId, ReadError, Value, property};
use common::refused;
use std::time::Duration;

const PRIORITY_ARRAY: u32 = 87;
const OUT_OF_SERVICE: u32 = 81;

fn present(client: &mut Client, device: &Device, object: ObjectId) -> Vec<Value> {
    client
        .read_property(device, object, property::PRESENT_VALUE, None)
        .unwrap()
}

fn write(
    client: &mut Client,
    device: &Device,
    object: ObjectId,
    prop: u32,
    value: Value,
    priority: Option<u8>,
) -> Result<(), ReadError> {
    client.write_property(
        device,
        object,
        PropertyRef::whole(prop),
        vec![value],
        priority,
    )
}

#[test]
fn the_highest_priority_slot_wins_and_relinquishing_falls_back() {
    let (mut c, d, _) = common::discover(Sim::lift_gateway());
    let pv = property::PRESENT_VALUE;
    write(&mut c, &d, sim::FAN, pv, Value::Real(30.0), Some(5)).unwrap();
    assert_eq!(
        present(&mut c, &d, sim::FAN),
        [Value::Real(30.0)],
        "slot 5 beats slot 8"
    );
    let slots = c.read_property(&d, sim::FAN, PRIORITY_ARRAY, None).unwrap();
    assert_eq!(
        (&slots[4], &slots[7]),
        (&Value::Real(30.0), &Value::Real(60.0))
    );

    write(&mut c, &d, sim::FAN, pv, Value::Null, Some(5)).unwrap();
    assert_eq!(
        present(&mut c, &d, sim::FAN),
        [Value::Real(60.0)],
        "back to slot 8"
    );
    write(&mut c, &d, sim::FAN, pv, Value::Null, Some(8)).unwrap();
    assert_eq!(
        present(&mut c, &d, sim::FAN),
        [Value::Real(0.0)],
        "relinquish-default"
    );
}

#[test]
fn an_input_takes_a_value_only_while_out_of_service() {
    let (mut c, d, _) = common::discover(Sim::lift_gateway());
    let pv = property::PRESENT_VALUE;
    let denied = write(&mut c, &d, sim::CAR_LOAD, pv, Value::Real(500.0), None);
    assert_eq!(
        refused(denied),
        (2, 40),
        "write-access-denied while in service"
    );

    write(
        &mut c,
        &d,
        sim::CAR_LOAD,
        OUT_OF_SERVICE,
        Value::Bool(true),
        None,
    )
    .unwrap();
    write(&mut c, &d, sim::CAR_LOAD, pv, Value::Real(500.0), None).unwrap();
    assert_eq!(present(&mut c, &d, sim::CAR_LOAD), [Value::Real(500.0)]);
}

#[test]
fn a_device_refuses_with_the_code_that_says_why() {
    let (mut c, d, _) = common::discover(Sim::lift_gateway());
    let id = Value::Object(sim::FAN);
    assert_eq!(refused(write(&mut c, &d, sim::FAN, 75, id, None)), (2, 40));
    let name = write(
        &mut c,
        &d,
        sim::FAN,
        property::OBJECT_NAME,
        Value::Real(1.0),
        None,
    );
    assert_eq!(refused(name), (2, 9), "invalid-data-type");
    let state = write(
        &mut c,
        &d,
        sim::MODE,
        property::PRESENT_VALUE,
        Value::Unsigned(9),
        None,
    );
    assert_eq!(refused(state), (2, 37), "value-out-of-range: four states");
    let gone = write(
        &mut c,
        &d,
        ObjectId::new(2, 77),
        property::OBJECT_NAME,
        Value::Null,
        None,
    );
    assert_eq!(refused(gone), (1, 31), "unknown-object");
}

#[test]
fn write_property_multiple_stops_at_the_first_refusal_and_keeps_what_went_before() {
    let (mut c, d, _) = common::discover(Sim::lift_gateway());
    let renamed = PropertyValue::new(property::OBJECT_NAME, vec![Value::Text("Fan".into())]);
    let forbidden = PropertyValue::new(75, vec![Value::Object(sim::FAN)]);
    let result = c.write_property_multiple(&d, &[(sim::FAN, vec![renamed, forbidden])]);
    assert_eq!(
        refused(result),
        (2, 40),
        "decoded through the wrapped error"
    );
    let name = c
        .read_property(&d, sim::FAN, property::OBJECT_NAME, None)
        .unwrap();
    assert_eq!(name, [Value::Text("Fan".into())]);
}

#[test]
fn a_written_value_reaches_a_cov_subscriber() {
    let (mut c, d, _) = common::discover(Sim::lift_gateway());
    c.subscribe_cov(&d, sim::FAN, 60).unwrap();
    let _initial = c.notifications(Duration::from_millis(300)).unwrap();
    write(
        &mut c,
        &d,
        sim::FAN,
        property::PRESENT_VALUE,
        Value::Real(75.0),
        Some(8),
    )
    .unwrap();
    let notes = c.notifications(Duration::from_millis(500)).unwrap();
    let seen = notes.iter().flat_map(|n| &n.values).any(|(p, v)| {
        p.property == property::PRESENT_VALUE && v.first() == Some(&Value::Real(75.0))
    });
    assert!(seen, "no notification carried the written value: {notes:?}");
}
