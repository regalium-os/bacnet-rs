// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

//! Every header spelling round-trips, and says whether it was the shortest.

use regalium_bacnet_core::tag::{Class, Header, Length, Lvt};

fn round_trip(h: Header) -> usize {
    let mut out = Vec::new();
    h.encode(&mut out);
    let len = usize::try_from(h.content_len()).unwrap();
    let with_content: Vec<u8> = out
        .iter()
        .copied()
        .chain(std::iter::repeat_n(0xAA, len))
        .collect();
    let (back, used) = Header::decode(&with_content, 0).unwrap();
    assert_eq!(back, h);
    assert_eq!(used, out.len());
    used
}

#[test]
fn every_tag_number_and_length_round_trips_and_is_canonical_when_built() {
    for number in [0, 1, 2, 14, 15, 64, 254] {
        for len in [0, 1, 4, 5, 253, 254, 0xFFFF, 0x1_0000] {
            for h in [
                Header::context(number, len),
                Header::application(number, len),
            ] {
                if h.class == Class::Application && number == 1 {
                    continue; // An application BOOLEAN has no length.
                }
                assert!(h.is_canonical(), "{h:?}");
                round_trip(h);
            }
        }
        for h in [Header::opening(number), Header::closing(number)] {
            assert!(h.is_canonical());
            assert_eq!(round_trip(h), if number >= 15 { 2 } else { 1 });
        }
    }
}

#[test]
fn a_long_spelling_decodes_reencodes_as_spelled_and_says_so() {
    for lvt in [
        Length::Extended8(3),
        Length::Extended16(3),
        Length::Extended32(3),
    ] {
        let h = Header {
            class: Class::Application,
            number: 2,
            extended_number: false,
            lvt: Lvt::Length(lvt),
        };
        assert!(!h.is_canonical());
        round_trip(h);
    }
    let escaped = Header {
        extended_number: true,
        ..Header::context(3, 1)
    };
    assert!(!escaped.is_canonical());
    assert_eq!(round_trip(escaped), 2);
}
