// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! BACnet/IPv6's BVLL: whatever decodes re-encodes byte for byte.

#![no_main]

use libfuzzer_sys::fuzz_target;
use regalium_bacnet::ipv6::bvll::Bvll6;

fuzz_target!(|data: &[u8]| {
    let Ok(b) = Bvll6::decode(data) else { return };
    let mut again = Vec::new();
    b.encode(&mut again);
    assert_eq!(again, data, "BVLL-IPv6 is lossless");
});
