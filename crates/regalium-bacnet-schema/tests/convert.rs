// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::panic,
    clippy::indexing_slicing
)]

//! From wire octets to the protobuf model: services the core decodes become
//! AIP resources, values keep the octets the device sent, and every name
//! the model issues reads back as the request that gets it.

use prost::Message;
use regalium_bacnet_core::apdu::Apdu;
use regalium_bacnet_core::primitive::ObjectIdentifier;
use regalium_bacnet_core::service::{IAm, ReadProperty, ReadPropertyAck, ServiceError};
use regalium_bacnet_schema::convert;
use regalium_bacnet_schema::device::Segmentation;
use regalium_bacnet_schema::object::value::Kind;

fn ack(apdu: &[u8]) -> ReadPropertyAck<'_> {
    ReadPropertyAck::decode(&apdu[3..]).unwrap()
}

#[test]
fn an_i_am_becomes_a_device_resource() {
    let body = [
        0xc4, 0x02, 0, 0x02, 0x57, 0x22, 0x05, 0xc4, 0x91, 0x03, 0x22, 0x01, 0x04,
    ];
    let d = convert::device(&IAm::decode(&body).unwrap());
    assert_eq!(
        (d.name.as_str(), d.instance, d.vendor_id, d.max_apdu_length),
        ("devices/599", 599, 260, 1476)
    );
    assert_eq!(d.segmentation(), Segmentation::None);
    assert_eq!(
        regalium_bacnet_schema::device::Device::decode(d.encode_to_vec().as_slice()).unwrap(),
        d
    );
}

#[test]
fn a_read_property_ack_becomes_a_property_that_keeps_the_octets_sent() {
    let wire = [
        0x30, 1, 0x0c, 0x0c, 0, 0, 0, 1, 0x19, 0x55, 0x3e, 0x44, 0x42, 0xc8, 0, 0, 0x3f,
    ];
    let p = convert::property(599, &ack(&wire)).unwrap();
    assert_eq!(
        p.name,
        "devices/599/objects/analog-input-1/properties/present-value"
    );
    let v = p.value.unwrap();
    assert_eq!(v.kind, Some(Kind::RealValue(100.0)));
    assert_eq!(v.encoded, [0x44, 0x42, 0xc8, 0, 0]);
}

#[test]
fn a_captured_object_name_reads_as_text() {
    let mut wire = vec![
        0x30, 2, 0x0c, 0x0c, 0x02, 0, 0x02, 0x57, 0x19, 0x4d, 0x3e, 0x75, 0x19, 0x00,
    ];
    wire.extend_from_slice(b"Lift gateway (simulated)");
    wire.push(0x3f);
    let p = convert::property(599, &ack(&wire)).unwrap();
    let Some(Kind::CharacterStringValue(s)) = p.value.unwrap().kind else {
        panic!("not text")
    };
    assert_eq!(
        (s.text.as_str(), s.character_set),
        ("Lift gateway (simulated)", 1)
    );
}

#[test]
fn a_priority_array_is_constructed_and_each_slot_keeps_its_octets() {
    let mut wire = vec![0x30, 4, 0x0c, 0x0c, 0, 0x40, 0, 1, 0x19, 0x57, 0x3e];
    wire.extend([0; 7]);
    wire.extend([0x44, 0x42, 0x70, 0, 0]);
    wire.extend([0; 8]);
    wire.push(0x3f);
    let v = convert::property(599, &ack(&wire)).unwrap().value.unwrap();
    let Some(Kind::ConstructedValue(c)) = v.kind else {
        panic!("not constructed")
    };
    assert_eq!(c.elements.len(), 16);
    let slot8 = c.elements[7].value.as_ref().unwrap();
    assert_eq!(
        (slot8.kind.clone(), slot8.encoded.clone()),
        (Some(Kind::RealValue(60.0)), vec![0x44, 0x42, 0x70, 0, 0])
    );
    assert_eq!(v.encoded.len(), 20);
}

#[test]
fn what_the_model_cannot_type_is_still_carried_in_the_encoded_octets() {
    // An Unsigned written long, and one above i64::MAX.
    let long = convert::value(&[0x25, 0x01, 0x48]).unwrap();
    assert_eq!(
        (long.kind, long.encoded),
        (Some(Kind::UnsignedValue(72)), vec![0x25, 0x01, 0x48])
    );
    let huge = [0x25, 0x08, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    let v = convert::value(&huge).unwrap();
    assert_eq!((v.kind, v.encoded), (None, huge.to_vec()));
    // A context-tagged element is carried raw, under its tag.
    let v = convert::value(&[0x0e, 0x19, 0x05, 0x0f]).unwrap();
    let Some(Kind::ConstructedValue(c)) = v.kind else {
        panic!("not constructed")
    };
    let Some(Kind::ConstructedValue(inner)) = c.elements[0].value.as_ref().unwrap().kind.clone()
    else {
        panic!("not nested")
    };
    assert_eq!(c.elements[0].context_tag, Some(0));
    assert_eq!(inner.elements[0].context_tag, Some(1));
    assert_eq!(
        inner.elements[0].value.as_ref().unwrap().kind,
        Some(Kind::OctetStringValue(vec![5]))
    );
}

#[test]
fn a_failed_read_is_a_property_carrying_its_error() {
    let read = ReadProperty {
        object: ObjectIdentifier {
            object_type: 0,
            instance: 99,
        },
        property: 85,
        index: None,
    };
    let p = convert::property_error(599, &read, &ServiceError { class: 1, code: 31 });
    let e = p.error.unwrap();
    assert_eq!(
        (e.error_class.as_str(), e.error_code.as_str()),
        ("object", "unknown-object")
    );
    assert!(p.value.is_none());
}

#[test]
fn an_apdu_decodes_straight_into_the_model() {
    let wire = [
        0x30, 1, 0x0c, 0x0c, 0, 0, 0, 1, 0x19, 0x55, 0x3e, 0x91, 0x3e, 0x3f,
    ];
    let apdu = Apdu::decode(&wire).unwrap();
    let regalium_bacnet_core::service::Service::ReadPropertyAck(a) =
        regalium_bacnet_core::service::Service::decode(&apdu).unwrap()
    else {
        panic!("not an ack")
    };
    assert_eq!(
        convert::property(1, &a).unwrap().value.unwrap().kind,
        Some(Kind::EnumeratedValue(62))
    );
}
