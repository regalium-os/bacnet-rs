// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A local device fed any request: it never panics, and what it sends back
//! is always an APDU that decodes.

#![no_main]

use libfuzzer_sys::fuzz_target;
use regalium_bacnet::apdu::Apdu;
use regalium_bacnet::obj::Memory;
use regalium_bacnet::primitive::{ObjectIdentifier, Primitive};
use regalium_bacnet::server::{Responder, Response};
use regalium_bacnet::tsm::Config;
use std::time::Duration;

fuzz_target!(|data: &[u8]| {
    let ao1 = ObjectIdentifier { object_type: 1, instance: 1 };
    let mut db = Memory::new(599, "Fuzzed", 1).object(ao1, "Point").commandable(ao1, &Primitive::Real(0.0));
    let mut r = Responder::new(Config::default());
    for (i, chunk) in data.split(|b| *b == 0xff).enumerate() {
        let Ok(apdu) = Apdu::decode(chunk) else { continue };
        let now = Duration::from_millis(u64::try_from(i).unwrap_or(0) * 500);
        let mut sent: Vec<Response> = r.handle(&0u8, &apdu, &mut db, now);
        sent.extend(r.poll(now).into_iter().map(|(_, s)| s));
        for s in sent {
            let (Response::Reply(b) | Response::Broadcast(b)) = s;
            assert!(Apdu::decode(&b).is_ok(), "a device sends only APDUs that decode");
        }
    }
});
