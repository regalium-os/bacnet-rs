// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

//! The primitive types: fewest octets out, the same value back in, from
//! either class of tag.

use regalium_bacnet_core::primitive::{
    BitString, CharacterString, Date, ObjectIdentifier, Primitive, Time,
};
use regalium_bacnet_core::tag::{Element, ErrorKind};

fn content(p: Primitive<'_>) -> usize {
    p.content().len()
}

#[test]
fn integers_take_the_fewest_octets_their_value_and_sign_need() {
    for (v, n) in [(0, 1), (255, 1), (256, 2), (65_535, 2), (u64::MAX, 8)] {
        assert_eq!(content(Primitive::Unsigned(v)), n, "unsigned {v}");
    }
    for (v, n) in [
        (0, 1),
        (1, 1),
        (-1, 1),
        (127, 1),
        (128, 2),
        (-128, 1),
        (-129, 2),
        (32_767, 2),
        (-32_768, 2),
        (i64::MAX, 8),
        (i64::MIN, 8),
    ] {
        assert_eq!(content(Primitive::Signed(v)), n, "signed {v}");
        let mut out = Vec::new();
        Primitive::Signed(v).encode(&mut out);
        let (e, _) = Element::decode(&out, 0).unwrap();
        assert_eq!(Primitive::decode(&e).unwrap(), Primitive::Signed(v));
    }
}

#[test]
fn every_primitive_round_trips_application_and_context_tagged() {
    let text = CharacterString {
        charset: 0,
        bytes: b"Car 1 load",
    };
    let values = [
        Primitive::Null,
        Primitive::Boolean(true),
        Primitive::Boolean(false),
        Primitive::Unsigned(1476),
        Primitive::Signed(-40),
        Primitive::Real(21.5),
        Primitive::Double(-0.125),
        Primitive::OctetString(&[1, 2, 3]),
        Primitive::CharacterString(text),
        Primitive::BitString(BitString {
            unused: 4,
            bytes: &[0xF0],
        }),
        Primitive::Enumerated(62),
        Primitive::Date(Date {
            year: 126,
            month: 10,
            day: 17,
            weekday: 6,
        }),
        Primitive::Time(Time {
            hour: 9,
            minute: 30,
            second: 0,
            hundredths: 255,
        }),
        Primitive::ObjectIdentifier(ObjectIdentifier {
            object_type: 8,
            instance: 599,
        }),
    ];
    for p in values {
        let mut app = Vec::new();
        p.encode(&mut app);
        let (e, used) = Element::decode(&app, 0).unwrap();
        assert_eq!((Primitive::decode(&e).unwrap(), used), (p, app.len()));
        let mut ctx = Vec::new();
        p.encode_context(3, &mut ctx);
        let (e, _) = Element::decode(&ctx, 0).unwrap();
        assert_eq!(
            Primitive::decode_as(p.tag(), e.content).unwrap(),
            p,
            "context {p:?}"
        );
        assert!(matches!(Primitive::decode(&e), Err(err) if err.kind == ErrorKind::WrongTag));
    }
}

#[test]
fn text_decodes_from_the_sets_it_names_and_refuses_what_it_cannot() {
    let latin = CharacterString {
        charset: 5,
        bytes: &[0x43, 0x61, 0x66, 0xE9],
    };
    assert_eq!(latin.text().unwrap(), "Café");
    let ucs2 = CharacterString {
        charset: 4,
        bytes: &[0x00, 0x43, 0x00],
    };
    assert_eq!(ucs2.text(), None, "an odd number of UCS-2 octets");
    assert_eq!(
        CharacterString {
            charset: 2,
            bytes: b"x"
        }
        .text(),
        None,
        "JIS is not decoded"
    );
}

#[test]
fn an_object_identifier_packs_ten_bits_of_type_and_twenty_two_of_instance() {
    let wildcard = ObjectIdentifier {
        object_type: 8,
        instance: ObjectIdentifier::WILDCARD_INSTANCE,
    };
    assert_eq!(wildcard.raw(), 0x023F_FFFF);
    assert_eq!(
        ObjectIdentifier::from_raw(0x00C0_000F),
        ObjectIdentifier {
            object_type: 3,
            instance: 15
        }
    );
}
