// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

//! The generated types on the protobuf wire: what the schema promises
//! survives an encode and a decode, and the names a consumer routes on are
//! the names in the .proto.

use prost::{Message, Name};
use regalium_bacnet_schema::device::{Address, Datalink, Device, DeviceState, Segmentation};
use regalium_bacnet_schema::object::value::Kind;
use regalium_bacnet_schema::object::{
    CharacterSet, Constructed, ConstructedElement, Object, ObjectId, Property, StatusFlags,
    Subscription, Value,
};

fn real(v: f32, encoded: &[u8]) -> Value {
    Value {
        kind: Some(Kind::RealValue(v)),
        encoded: encoded.to_vec(),
    }
}

fn round_trip<M: Message + Default + PartialEq + std::fmt::Debug>(m: &M) {
    let back = M::decode(m.encode_to_vec().as_slice()).unwrap();
    assert_eq!(&back, m);
}

#[test]
fn a_value_keeps_the_octets_the_device_sent_beside_what_they_mean() {
    // 72.0 as an application-tagged REAL, §20.2.6: tag 4, length 4.
    let value = real(72.0, &[0x44, 0x42, 0x90, 0x00, 0x00]);
    round_trip(&value);
    assert_eq!(value.encoded.first(), Some(&0x44));
}

#[test]
fn a_constructed_value_nests_values_with_their_context_tags() {
    let inner = Value {
        kind: Some(Kind::ObjectIdValue(ObjectId {
            object_type: "analog-input".into(),
            instance: 1,
        })),
        encoded: Vec::new(),
    };
    let value = Value {
        kind: Some(Kind::ConstructedValue(Constructed {
            elements: vec![
                ConstructedElement {
                    context_tag: Some(0),
                    value: Some(inner),
                },
                ConstructedElement {
                    context_tag: None,
                    value: Some(real(21.5, &[])),
                },
            ],
        })),
        encoded: Vec::new(),
    };
    round_trip(&value);
}

#[test]
fn a_device_round_trips_with_its_address_and_state() {
    let device = Device {
        name: "devices/599".into(),
        instance: 599,
        display_name: "Lift gateway".into(),
        vendor_id: 999,
        protocol_revision: 19,
        max_apdu_length: 1476,
        segmentation: Segmentation::None.into(),
        state: DeviceState::Reachable.into(),
        address: Some(Address {
            datalink: Datalink::BacnetIp.into(),
            endpoint: "10.0.0.5:47808".into(),
            network_number: 2001,
            mac_address: vec![3],
        }),
        ..Device::default()
    };
    round_trip(&device);
}

#[test]
fn an_object_carries_its_states_names_and_status_flags() {
    let object = Object {
        name: "devices/599/objects/multi-state-value-1".into(),
        object_type: "multi-state-value".into(),
        instance: 1,
        state_texts: vec![
            "Out of service".into(),
            "Normal".into(),
            "Independent".into(),
        ],
        status_flags: Some(StatusFlags {
            alarm: true,
            ..StatusFlags::default()
        }),
        ..Object::default()
    };
    round_trip(&object);
}

#[test]
fn every_resource_names_its_proto_type_for_routing() {
    assert_eq!(Device::full_name(), "bacnet.device.v1.Device");
    assert_eq!(Object::full_name(), "bacnet.object.v1.Object");
    assert_eq!(Property::full_name(), "bacnet.object.v1.Property");
    assert_eq!(Subscription::full_name(), "bacnet.object.v1.Subscription");
}

#[test]
fn a_character_set_is_its_wire_octet_plus_one() {
    // §20.2.9 gives UTF-8 octet 0, which protobuf reserves for unspecified.
    assert_eq!(CharacterSet::Utf8 as i32, 1);
    assert_eq!(CharacterSet::Iso88591 as i32, 6);
}
