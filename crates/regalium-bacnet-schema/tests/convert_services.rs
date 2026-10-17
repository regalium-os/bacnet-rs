// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! The services beyond `ReadProperty` through the model: a multiple read
//! and a notification in, a subscription and a write out — and a value read
//! in, written back from its typed form alone, as the octets it came as.

use regalium_bacnet_core::service::{CovNotification, ReadPropertyMultipleAck};
use regalium_bacnet_schema::convert::{self, CovRequest};
use regalium_bacnet_schema::object::value::Kind;
use regalium_bacnet_schema::object::{Property, Subscription, Value};

fn octets(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

#[test]
fn a_multiple_read_becomes_a_property_per_result_errors_included() {
    let body = octets("0c 00 00 00 01 1e 29 55 4e 44 42 c8 00 00 4f 29 1c 5e 91 02 91 20 5f 1f");
    let all = convert::properties(599, &ReadPropertyMultipleAck::decode(&body).unwrap()).unwrap();
    assert_eq!(
        all[0].name,
        "devices/599/objects/analog-input-1/properties/present-value"
    );
    assert_eq!(
        all[0].value.as_ref().unwrap().kind,
        Some(Kind::RealValue(100.0))
    );
    assert_eq!(
        all[1].name,
        "devices/599/objects/analog-input-1/properties/description"
    );
    assert_eq!(
        all[1].error.as_ref().unwrap().error_code,
        "unknown-property"
    );
}

#[test]
fn a_notification_names_its_subscription_and_each_value() {
    let body =
        octets("09 07 1c 02 00 02 57 2c 00 00 00 01 3a 01 22 4e 09 55 2e 44 41 ac 00 00 2f 4f");
    let n = convert::notification(&CovNotification::decode(&body).unwrap()).unwrap();
    assert_eq!(
        n.subscription,
        "devices/599/objects/analog-input-1/subscriptions/7"
    );
    assert_eq!(
        n.values[0].name,
        "devices/599/objects/analog-input-1/properties/present-value"
    );
    assert_eq!(n.time_remaining.unwrap().seconds, 290);
}

#[test]
fn a_subscription_is_the_subscribe_cov_that_makes_it() {
    let mut s = Subscription {
        name: "devices/599/objects/analog-input-1/subscriptions/7".into(),
        confirmed: true,
        ..Subscription::default()
    };
    let (device, CovRequest::Object(r)) = convert::subscribe(&s).unwrap() else {
        panic!()
    };
    assert_eq!(
        (device, r.process, r.confirmed, r.lifetime),
        (599, 7, Some(true), Some(120))
    );
    s.property = "present-value".into();
    s.increment = Some(0.5);
    let (_, CovRequest::Property(p)) = convert::subscribe(&s).unwrap() else {
        panic!()
    };
    assert_eq!((p.property.property, p.increment), (85, Some(0.5)));
    s.name = "devices/599/objects/analog-input-1".into();
    assert!(convert::subscribe(&s).is_none());
}

#[test]
fn an_updated_property_is_the_write_property_that_sets_it() {
    let p = Property {
        name: "devices/599/objects/analog-output-1/properties/present-value".into(),
        value: Some(Value {
            kind: Some(Kind::RealValue(42.5)),
            encoded: Vec::new(),
        }),
        write_priority: 8,
        ..Property::default()
    };
    let w = convert::write(&p).unwrap();
    let mut body = Vec::new();
    w.request().encode(&mut body);
    assert_eq!(
        body,
        octets("0c 00 40 00 01 19 55 3e 44 42 2a 00 00 3f 49 08")
    );
    // Octets the model carries are written as they are, long spelling and all.
    let long = Value {
        kind: Some(Kind::UnsignedValue(72)),
        encoded: vec![0x25, 0x01, 0x48],
    };
    assert_eq!(convert::encode_value(&long).unwrap(), [0x25, 0x01, 0x48]);
}

#[test]
fn a_value_read_in_is_written_back_from_its_type_alone_as_the_same_octets() {
    for wire in [
        "44 42 c8 00 00",
        "21 05",
        "32 ff 01",
        "75 06 00 4c 69 66 74 31",
        "82 04 a0",
        "91 3e",
        "c4 02 00 02 57",
        "a4 7e 0a 11 04",
        "b4 0e 1e 00 00",
        "00 00 00 00 00 00 00 44 42 70 00 00 00 00 00 00 00 00 00 00",
        "0e 19 05 0f",
        "10 11",
    ] {
        let original = octets(wire);
        let mut v = convert::value(&original).unwrap_or_else(|e| panic!("{wire}: {e}"));
        strip(&mut v);
        assert_eq!(convert::encode_value(&v), Some(original), "{wire}");
    }
}

/// Drops every `encoded`, so only the typed value is left to write from.
fn strip(v: &mut Value) {
    v.encoded.clear();
    if let Some(Kind::ConstructedValue(c)) = &mut v.kind {
        c.elements
            .iter_mut()
            .filter_map(|e| e.value.as_mut())
            .for_each(strip);
    }
}
