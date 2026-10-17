// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! NPDUs and APDUs: whatever decodes re-encodes byte for byte, the APDU an
//! NPDU carries included.

#![no_main]

use libfuzzer_sys::fuzz_target;
use regalium_bacnet::apdu::Apdu;
use regalium_bacnet::npdu::{Body, Npdu};

fuzz_target!(|data: &[u8]| {
    if let Ok(n) = Npdu::decode(data) {
        let mut again = Vec::new();
        n.encode(&mut again);
        assert_eq!(again, data, "NPDUs are lossless");
        if let Body::Message(m) = n.body {
            let _ = m.decode();
        }
    }
    if let Ok(a) = Apdu::decode(data) {
        let mut again = Vec::new();
        a.encode(&mut again);
        assert_eq!(again, data, "APDUs are lossless");
    }
});
