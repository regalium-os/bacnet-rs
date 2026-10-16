// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! Creating and deleting objects, against the simulated gateway over real
//! UDP: what a controller with a dynamic database allows, and the error code
//! for each thing it refuses.

mod common;

use bacnet_probe::service::object::Specifier;
use bacnet_probe::service::write::PropertyValue;
use bacnet_probe::sim::{self, Sim};
use bacnet_probe::{ObjectId, ReadError, Value, property};
use common::refused;

#[test]
fn an_object_is_created_with_initial_values_and_deleted_again() {
    let (mut c, d, _) = common::discover(Sim::lift_gateway());
    let initial = [
        PropertyValue::new(property::OBJECT_NAME, vec![Value::Text("Setpoint".into())]),
        PropertyValue::new(property::PRESENT_VALUE, vec![Value::Real(21.5)]),
    ];
    let id = c.create_object(&d, Specifier::Type(2), &initial).unwrap();
    assert_eq!(
        id,
        ObjectId::new(2, 1),
        "the device chose the first free instance"
    );
    assert_eq!(
        c.read_property(&d, id, property::PRESENT_VALUE, None)
            .unwrap(),
        [Value::Real(21.5)]
    );
    let list = c
        .read_property(&d, sim::DEVICE, property::OBJECT_LIST, None)
        .unwrap();
    assert!(list.contains(&Value::Object(id)));

    c.delete_object(&d, id).unwrap();
    let read = c.read_property(&d, id, property::PRESENT_VALUE, None);
    assert!(matches!(
        read,
        Err(ReadError::Bacnet { class: 1, code: 31 })
    ));
}

#[test]
fn creation_and_deletion_are_refused_where_a_controller_would_refuse_them() {
    let (mut c, d, _) = common::discover(Sim::lift_gateway());
    let taken = c.create_object(&d, Specifier::Id(sim::FAN), &[]).map(drop);
    assert_eq!(refused(taken), (1, 24), "object-identifier-already-exists");
    let loop_ = c.create_object(&d, Specifier::Type(12), &[]).map(drop);
    assert_eq!(refused(loop_), (1, 36), "unsupported-object-type");
    let bad = [PropertyValue::new(
        property::PRESENT_VALUE,
        vec![Value::Text("x".into())],
    )];
    let rolled_back = c.create_object(&d, Specifier::Type(2), &bad).map(drop);
    assert_eq!(
        refused(rolled_back),
        (2, 9),
        "an initial value is checked like a write"
    );
    assert_eq!(
        refused(c.delete_object(&d, sim::FAN)),
        (1, 23),
        "shipped objects stay"
    );
}
