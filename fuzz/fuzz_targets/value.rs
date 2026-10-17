// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A value into the protobuf model and back: whatever the model takes from
//! the wire, it writes back exactly.

#![no_main]

use libfuzzer_sys::fuzz_target;
use regalium_bacnet::schema::convert;

fuzz_target!(|data: &[u8]| {
    let Ok(v) = convert::value(data) else { return };
    assert_eq!(convert::encode_value(&v).as_deref(), Some(data), "the model keeps the octets");
});
