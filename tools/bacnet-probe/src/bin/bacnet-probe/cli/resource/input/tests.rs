// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::float_cmp)]

//! Typed input: the same text becomes whatever the property holds.

use super::parse;
use regalium_bacnet_schema::object::{self as obj, value::Kind};

fn holding(kind: Kind) -> obj::Value {
    obj::Value {
        kind: Some(kind),
        encoded: Vec::new(),
    }
}

fn kind(text: &str, current: Option<Kind>, states: &[&str]) -> Kind {
    let states: Vec<String> = states.iter().map(|s| (*s).to_owned()).collect();
    parse(text, current.map(holding).as_ref(), &states)
        .unwrap()
        .kind
        .unwrap()
}

#[test]
fn a_number_takes_the_type_the_property_already_holds() {
    assert_eq!(
        kind("75", Some(Kind::RealValue(0.0)), &[]),
        Kind::RealValue(75.0)
    );
    assert_eq!(
        kind("75", Some(Kind::UnsignedValue(0)), &[]),
        Kind::UnsignedValue(75)
    );
    assert_eq!(
        kind("75", Some(Kind::EnumeratedValue(0)), &[]),
        Kind::EnumeratedValue(75)
    );
}

#[test]
fn a_state_is_written_by_its_name() {
    let modes = ["Out of service", "Normal", "Independent"];
    assert_eq!(
        kind("independent", Some(Kind::UnsignedValue(2)), &modes),
        Kind::UnsignedValue(3)
    );
    let door = ["closed", "open"];
    assert_eq!(
        kind("open", Some(Kind::EnumeratedValue(0)), &door),
        Kind::EnumeratedValue(1)
    );
    assert_eq!(
        kind("inactive", Some(Kind::EnumeratedValue(1)), &[]),
        Kind::EnumeratedValue(0)
    );
}

#[test]
fn null_relinquishes_whatever_the_type() {
    assert!(matches!(
        kind("null", Some(Kind::RealValue(1.0)), &[]),
        Kind::NullValue(_)
    ));
}

#[test]
fn text_that_cannot_be_the_type_is_refused_with_the_reason() {
    let refused = parse("fast", Some(&holding(Kind::RealValue(0.0))), &[]).unwrap_err();
    assert!(refused.contains("not a number"), "{refused}");
}

#[test]
fn with_nothing_to_go_by_the_text_is_read_for_what_it_looks_like() {
    assert_eq!(kind("21.5", None, &[]), Kind::RealValue(21.5));
    assert_eq!(kind("3", None, &[]), Kind::UnsignedValue(3));
    assert_eq!(kind("true", None, &[]), Kind::BooleanValue(true));
    assert!(matches!(
        kind("Setpoint", None, &[]),
        Kind::CharacterStringValue(_)
    ));
}
