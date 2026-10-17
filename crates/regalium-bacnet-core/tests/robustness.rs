// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    clippy::indexing_slicing
)]

//! Malformed input is the normal case on a shared wire: errors point at the
//! octets that failed, and no input at all makes the codec panic.

use regalium_bacnet_core::primitive::Primitive;
use regalium_bacnet_core::tag::{self, ErrorKind};

#[test]
fn an_error_names_the_offset_and_carries_the_octets_that_failed() {
    let buf = [0x21, 0x48, 0x44, 0x42];
    let err = tag::decode_all(&buf).unwrap_err();
    assert_eq!(
        err.kind,
        ErrorKind::Truncated {
            needed: 5,
            available: 2
        }
    );
    assert_eq!((err.offset, err.bytes), (2, &buf[2..]));
    assert!(err.to_string().contains("at offset 2 44 42"), "{err}");
}

#[test]
fn no_input_makes_the_codec_panic_and_whatever_decodes_round_trips() {
    // A deterministic sweep: the same buffers every run, so a failure here
    // is a fixture to add, not a flake to rerun.
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..50_000 {
        let len = usize::try_from(next() % 24).unwrap();
        let buf: Vec<u8> = (0..len).map(|_| next().to_le_bytes()[0]).collect();
        for e in tag::elements(&buf).flatten() {
            let _ = Primitive::decode(&e);
            for t in 0..14 {
                let _ = Primitive::decode_as(t, e.content);
            }
        }
        if let Ok(elements) = tag::decode_all(&buf) {
            assert_eq!(tag::encode_all(&elements), buf, "lossless on {buf:02x?}");
        }
    }
}
