// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! BACnet/IP's BVLL: whatever decodes re-encodes byte for byte, and its
//! tables iterate without panic.

#![no_main]

use libfuzzer_sys::fuzz_target;
use regalium_bacnet::ip::bvll::{Bvll, Function};

fuzz_target!(|data: &[u8]| {
    let Ok(b) = Bvll::decode(data) else { return };
    let mut again = Vec::new();
    b.encode(&mut again);
    assert_eq!(again, data, "BVLL is lossless");
    match b.function {
        Function::WriteBdt(t) | Function::ReadBdtAck(t) => t.iter().for_each(drop),
        Function::ReadFdtAck(t) => t.iter().for_each(drop),
        _ => {}
    }
});
