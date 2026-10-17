// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Tags and primitives: anything that decodes re-encodes byte for byte, and
//! reading an element as a primitive never panics.

#![no_main]

use libfuzzer_sys::fuzz_target;
use regalium_bacnet::primitive::Primitive;
use regalium_bacnet::tag;

fuzz_target!(|data: &[u8]| {
    if let Ok(elements) = tag::decode_all(data) {
        assert_eq!(tag::encode_all(&elements), data, "tags are lossless");
        for e in &elements {
            let _ = Primitive::decode(e);
        }
    }
});
