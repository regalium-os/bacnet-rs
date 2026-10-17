// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! The API's writes against the simulated gateway over real UDP: updating
//! properties and objects, creating and deleting, and how each refusal
//! reaches the caller.

mod common;

use bacnet_probe::api::{Api, Code, name};
use bacnet_probe::sim;
use prost_types::{FieldMask, NullValue};
use regalium_bacnet_schema::object::{self as obj, value::Kind};

fn value(kind: Kind) -> obj::Value {
    obj::Value {
        kind: Some(kind),
        encoded: Vec::new(),
    }
}

fn write_pv(
    api: &mut Api,
    kind: Kind,
    priority: i32,
) -> Result<obj::Property, bacnet_probe::api::ApiError> {
    let property = obj::Property {
        name: name::property(599, sim::FAN, 85),
        value: Some(value(kind)),
        write_priority: priority,
        ..Default::default()
    };
    api.update_property(&obj::UpdatePropertyRequest {
        property: Some(property),
        update_mask: None,
    })
}

#[test]
fn a_property_update_returns_the_winner_of_the_priority_array() {
    let mut api = common::api();
    let won = write_pv(&mut api, Kind::RealValue(30.0), 5).unwrap();
    assert_eq!(won.value.unwrap().kind, Some(Kind::RealValue(30.0)));
    let back = write_pv(&mut api, Kind::NullValue(NullValue::NullValue.into()), 5).unwrap();
    assert_eq!(
        back.value.unwrap().kind,
        Some(Kind::RealValue(60.0)),
        "slot 8 again"
    );
    let lost = write_pv(&mut api, Kind::RealValue(10.0), 12).unwrap();
    assert_eq!(
        lost.value.unwrap().kind,
        Some(Kind::RealValue(60.0)),
        "slot 12 loses to 8"
    );
}

#[test]
fn an_object_update_writes_only_what_the_mask_names() {
    let mut api = common::api();
    let object = obj::Object {
        name: name::object(599, sim::MODE),
        display_name: "Should not be written".into(),
        present_value: Some(value(Kind::UnsignedValue(3))),
        ..Default::default()
    };
    let mask = FieldMask {
        paths: vec!["present_value".into()],
    };
    let req = obj::UpdateObjectRequest {
        object: Some(object),
        update_mask: Some(mask),
    };
    let after = api.update_object(&req).unwrap();
    assert_eq!(
        after.present_value.unwrap().kind,
        Some(Kind::UnsignedValue(3))
    );
    assert_eq!(after.display_name, "Car 1 operating mode");
}

#[test]
fn an_object_is_created_read_and_deleted_through_the_api() {
    let mut api = common::api();
    let object = obj::Object {
        object_type: "analog-value".into(),
        display_name: "Setpoint".into(),
        present_value: Some(value(Kind::RealValue(21.5))),
        ..Default::default()
    };
    let req = obj::CreateObjectRequest {
        parent: "devices/599".into(),
        object: Some(object),
        object_id: String::new(),
    };
    let made = api.create_object(&req).unwrap();
    assert_eq!(made.name, "devices/599/objects/analog-value-1");
    assert_eq!(
        (made.display_name.as_str(), made.present_value.unwrap().kind),
        ("Setpoint", Some(Kind::RealValue(21.5)))
    );

    api.delete_object(&obj::DeleteObjectRequest {
        name: made.name.clone(),
    })
    .unwrap();
    let gone = api
        .get_object(&obj::GetObjectRequest { name: made.name })
        .unwrap_err();
    assert_eq!(gone.code, Code::NotFound);
}

#[test]
fn each_refusal_reaches_the_caller_as_the_status_it_means() {
    let mut api = common::api();
    let wrong_type = write_pv(&mut api, Kind::BooleanValue(true), 8).unwrap_err();
    assert_eq!(
        (wrong_type.code, wrong_type.bacnet),
        (Code::InvalidArgument, Some((2, 9)))
    );
    let input = obj::Property {
        name: name::property(599, sim::CAR_LOAD, 85),
        value: Some(value(Kind::RealValue(1.0))),
        ..Default::default()
    };
    let in_service = api
        .update_property(&obj::UpdatePropertyRequest {
            property: Some(input),
            update_mask: None,
        })
        .unwrap_err();
    assert_eq!(
        in_service.code,
        Code::FailedPrecondition,
        "an input in service"
    );

    let create = |api: &mut Api, kind: &str, id: &str| {
        let object = obj::Object {
            object_type: kind.into(),
            ..Default::default()
        };
        let req = obj::CreateObjectRequest {
            parent: "devices/599".into(),
            object: Some(object),
            object_id: id.into(),
        };
        api.create_object(&req).unwrap_err().code
    };
    assert_eq!(
        create(&mut api, "analog-output", "analog-output-1"),
        Code::AlreadyExists
    );
    assert_eq!(create(&mut api, "loop", ""), Code::Unimplemented);
    assert_eq!(
        create(&mut api, "analog-value", "binary-value-1"),
        Code::InvalidArgument
    );
    let shipped = api
        .delete_object(&obj::DeleteObjectRequest {
            name: name::object(599, sim::FAN),
        })
        .unwrap_err();
    assert_eq!(shipped.code, Code::FailedPrecondition);
    let mask = obj::UpdateObjectRequest {
        object: Some(obj::Object {
            name: name::object(599, sim::FAN),
            ..Default::default()
        }),
        update_mask: Some(FieldMask {
            paths: vec!["units".into()],
        }),
    };
    assert_eq!(
        api.update_object(&mask).unwrap_err().code,
        Code::InvalidArgument
    );
}
