// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! The API's reads against the simulated gateway over real UDP: discovery,
//! devices, objects and properties, as the schema's messages.

mod common;

use bacnet_probe::api::{Code, name};
use bacnet_probe::{ObjectId, sim};
use regalium_bacnet_schema::device::{self as dev, DeviceState, Segmentation};
use regalium_bacnet_schema::object::{self as obj, value::Kind};

#[test]
fn discovery_lists_what_answered_and_get_device_reads_it_now() {
    let mut api = common::api();
    let found = api
        .discover_devices(&dev::DiscoverDevicesRequest::default())
        .unwrap();
    assert_eq!(found.devices.len(), 1);
    assert_eq!(found.devices[0].name, "devices/599");
    let listed = api
        .list_devices(&dev::ListDevicesRequest::default())
        .unwrap();
    assert_eq!(listed.devices.len(), 1);

    let d = api
        .get_device(&dev::GetDeviceRequest {
            name: "devices/599".into(),
        })
        .unwrap();
    assert_eq!(d.display_name, "Lift gateway (simulated)");
    assert_eq!(
        (d.vendor.as_str(), d.vendor_id),
        ("RegaliumOS simulator", 999)
    );
    assert_eq!(
        (d.protocol_revision, d.max_apdu_length, d.object_count),
        (22, 1476, 6)
    );
    assert_eq!(d.segmentation(), Segmentation::None);
    assert_eq!(d.state(), DeviceState::Reachable);
    assert!(d.address.unwrap().endpoint.starts_with("127.0.0.1:"));
}

#[test]
fn an_object_carries_its_units_states_health_and_priority_array() {
    let mut api = common::api();
    let get = |api: &mut bacnet_probe::api::Api, id| {
        api.get_object(&obj::GetObjectRequest {
            name: name::object(599, id),
        })
        .unwrap()
    };
    let fan = get(&mut api, sim::FAN);
    assert_eq!(
        (fan.object_type.as_str(), fan.units.as_str()),
        ("analog-output", "percent")
    );
    assert_eq!(fan.priority_array.len(), 16);
    assert_eq!(fan.priority_array[7].kind, Some(Kind::RealValue(60.0)));
    let mode = get(&mut api, sim::MODE);
    assert_eq!(
        mode.state_texts,
        ["Out of service", "Normal", "Independent", "Fire service"]
    );
    assert_eq!(
        mode.present_value.unwrap().kind,
        Some(Kind::UnsignedValue(2))
    );
    let room = get(&mut api, sim::MACHINE_ROOM);
    assert!(
        room.status_flags.unwrap().alarm,
        "the machine room is in alarm"
    );
    assert_eq!(room.units, "degrees-celsius");
    assert_eq!(
        room.event_state, "high-limit",
        "41.5 °C trips the high limit"
    );
}

#[test]
fn listing_pages_through_the_object_list_and_ends() {
    let mut api = common::api();
    let mut names = Vec::new();
    let mut token = String::new();
    for _ in 0..10 {
        let req = obj::ListObjectsRequest {
            parent: "devices/599".into(),
            page_size: 4,
            page_token: token,
            ..Default::default()
        };
        let page = api.list_objects(&req).unwrap();
        names.extend(page.objects.into_iter().map(|o| o.name));
        token = page.next_page_token;
        if token.is_empty() {
            break;
        }
    }
    assert_eq!(names.len(), 6, "{names:?}");
    assert!(names.contains(&"devices/599/objects/binary-input-1".to_owned()));
}

#[test]
fn a_batch_keeps_each_propertys_value_or_error_in_the_order_asked() {
    let mut api = common::api();
    let names = [85, 512, 77]
        .map(|p| name::property(599, sim::FAN, p))
        .to_vec();
    let req = obj::BatchGetPropertiesRequest {
        parent: name::object(599, sim::FAN),
        names,
    };
    let got = api.batch_get_properties(&req).unwrap().properties;
    assert!(got[1].name.ends_with("/properties/property-512"));
    assert_eq!(
        got[1].error.as_ref().unwrap().error_code,
        "unknown-property"
    );
    assert_eq!(
        got[2].value.as_ref().unwrap().kind.as_ref().map(|_| ()),
        Some(())
    );
    let listed = api
        .list_properties(&obj::ListPropertiesRequest {
            parent: name::object(599, sim::FAN),
            ..Default::default()
        })
        .unwrap();
    assert!(
        listed
            .properties
            .iter()
            .any(|p| p.name.ends_with("/priority-array"))
    );
}

#[test]
fn who_has_finds_an_object_by_name_on_every_device() {
    let mut api = common::api();
    let req = obj::FindObjectsRequest {
        parent: "devices/-".into(),
        target: Some(obj::find_objects_request::Target::DisplayName(
            "Car 1 door".into(),
        )),
        wait: None,
    };
    let found = api.find_objects(&req).unwrap().objects;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, name::object(599, sim::DOOR));
}

#[test]
fn a_failure_carries_the_status_a_caller_acts_on() {
    let mut api = common::api();
    let missing = api
        .get_object(&obj::GetObjectRequest {
            name: name::object(599, ObjectId::new(2, 77)),
        })
        .unwrap_err();
    assert_eq!(
        (missing.code, missing.bacnet),
        (Code::NotFound, Some((1, 31)))
    );
    let bad = api
        .get_object(&obj::GetObjectRequest {
            name: "devices/599/objects/0-1".into(),
        })
        .unwrap_err();
    assert_eq!(bad.code, Code::InvalidArgument);
    let silent = api
        .get_device(&dev::GetDeviceRequest {
            name: "devices/123".into(),
        })
        .unwrap_err();
    assert_eq!(silent.code, Code::NotFound, "no device 123 answered Who-Is");
    let filtered = api.list_objects(&obj::ListObjectsRequest {
        parent: "devices/599".into(),
        filter: "x".into(),
        ..Default::default()
    });
    assert_eq!(filtered.unwrap_err().code, Code::Unimplemented);
}
