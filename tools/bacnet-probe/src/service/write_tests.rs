// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

//! The change codecs byte for byte: a vector from the standard, then each
//! request decoding back to what was encoded.

use super::PropertyRef;
use super::object::{self, Specifier};
use super::write::{self, PropertyValue};
use crate::tag::{ObjectId, Value};

const ANALOG_VALUE_1: ObjectId = ObjectId::new(2, 1);

#[test]
fn write_property_encodes_as_the_standards_example() {
    // ASHRAE 135 Annex F, WriteProperty: 180.0 to analog-value 1's
    // present-value, no priority.
    let w = PropertyValue::new(85, vec![Value::Real(180.0)]);
    let want = [
        0x0C, 0x00, 0x80, 0x00, 0x01, 0x19, 0x55, 0x3E, 0x44, 0x43, 0x34, 0x00, 0x00, 0x3F,
    ];
    assert_eq!(write::request(ANALOG_VALUE_1, &w), want);
    assert_eq!(write::decode_request(&want), Some((ANALOG_VALUE_1, w)));
}

#[test]
fn a_relinquish_carries_null_and_its_priority() {
    let w = PropertyValue {
        property: PropertyRef::whole(85),
        values: vec![Value::Null],
        priority: Some(8),
    };
    let body = write::request(ANALOG_VALUE_1, &w);
    assert!(
        body.ends_with(&[0x3E, 0x00, 0x3F, 0x49, 0x08]),
        "{body:02x?}"
    );
    assert_eq!(write::decode_request(&body), Some((ANALOG_VALUE_1, w)));
}

#[test]
fn trailing_octets_after_a_write_are_malformed_not_ignored() {
    let mut body = write::request(ANALOG_VALUE_1, &PropertyValue::new(85, vec![Value::Null]));
    body.push(0x00);
    assert_eq!(write::decode_request(&body), None);
}

#[test]
fn write_property_multiple_round_trips_several_objects() {
    let writes = vec![
        (
            ANALOG_VALUE_1,
            vec![PropertyValue::new(77, vec![Value::Text("Setpoint".into())])],
        ),
        (
            ObjectId::new(5, 2),
            vec![
                PropertyValue::new(81, vec![Value::Bool(true)]),
                PropertyValue {
                    property: PropertyRef {
                        property: 85,
                        index: None,
                    },
                    values: vec![Value::Enumerated(1)],
                    priority: Some(16),
                },
            ],
        ),
    ];
    let body = write::request_multiple(&writes);
    assert_eq!(write::decode_request_multiple(&body), Some(writes));
}

#[test]
fn create_object_round_trips_by_type_and_by_identifier() {
    let initial = vec![PropertyValue::new(77, vec![Value::Text("Setpoint".into())])];
    for what in [Specifier::Type(2), Specifier::Id(ANALOG_VALUE_1)] {
        let body = object::create_request(what, &initial);
        assert_eq!(
            object::decode_create_request(&body),
            Some((what, initial.clone()))
        );
    }
    let bare = object::create_request(Specifier::Type(5), &[]);
    assert_eq!(
        object::decode_create_request(&bare),
        Some((Specifier::Type(5), Vec::new()))
    );
}

#[test]
fn an_object_identifier_body_is_application_tagged() {
    let body = object::object_body(ANALOG_VALUE_1);
    assert_eq!(body, [0xC4, 0x00, 0x80, 0x00, 0x01]);
    assert_eq!(object::decode_object_body(&body), Some(ANALOG_VALUE_1));
}

#[test]
fn an_error_decodes_plain_or_wrapped() {
    // Plain, as ReadProperty answers: property, unknown-property.
    assert_eq!(
        object::decode_error(&[0x91, 0x02, 0x91, 0x20]),
        Some((2, 32))
    );
    // Wrapped, as CreateObject answers, with the failed element after it.
    let wrapped = object::constructed_error(1, 24, &[0x19, 0x00]);
    assert_eq!(object::decode_error(&wrapped), Some((1, 24)));
    assert_eq!(
        object::decode_error(&[0x0E, 0x91, 0x01]),
        None,
        "unclosed wrapper"
    );
}
