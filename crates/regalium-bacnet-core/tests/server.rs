// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! The server's gate: every record of `fixtures/server/` is a request and the
//! exact APDU a local device answers with — or `none` — replayed in order
//! against one database, so writes and the reads after them are checked
//! together.

use regalium_bacnet_core::apdu::Apdu;
use regalium_bacnet_core::obj::Memory;
use regalium_bacnet_core::primitive::{BitString, ObjectIdentifier, Primitive};
use regalium_bacnet_core::server::{Response, respond};

/// Every corpus. `just fixtures` fails when one on disk is missing here.
const CORPORA: &[(&str, &str)] = &[("device.hex", include_str!("../fixtures/server/device.hex"))];

const AI1: ObjectIdentifier = ObjectIdentifier {
    object_type: 0,
    instance: 1,
};
const AO1: ObjectIdentifier = ObjectIdentifier {
    object_type: 1,
    instance: 1,
};
const AV1: ObjectIdentifier = ObjectIdentifier {
    object_type: 2,
    instance: 1,
};

/// The database the corpus header describes.
fn database() -> Memory {
    Memory::new(599, "Lift gateway", 260)
        .object(AI1, "Car 1 load")
        .with(AI1, 85, &Primitive::Real(420.0))
        .with(AI1, 117, &Primitive::Enumerated(39))
        .with(
            AI1,
            111,
            &Primitive::BitString(BitString {
                unused: 4,
                bytes: &[0],
            }),
        )
        .object(AO1, "Fan speed")
        .commandable(AO1, &Primitive::Real(0.0))
        .object(AV1, "Setpoint")
        .with(AV1, 85, &Primitive::Real(21.5))
        .writable(AV1, 85)
}

fn octets(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("hex octet"))
        .collect()
}

fn spell(o: &[u8]) -> String {
    o.iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn every_server_fixture_record_is_answered_exactly() {
    assert!(!CORPORA.is_empty(), "a walker that finds nothing must fail");
    let mut failures = Vec::new();
    let mut total = 0;
    for (file, text) in CORPORA {
        let mut db = database();
        let mut records = 0;
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (input, want) = line
                .split_once(" => ")
                .unwrap_or_else(|| panic!("{file}:{}: no ' => '", n + 1));
            let request = octets(input);
            let got = match respond(&Apdu::decode(&request).unwrap(), &mut db) {
                None => "none".to_owned(),
                Some(Response::Reply(r)) => format!("reply {}", spell(&r)),
                Some(Response::Broadcast(r)) => format!("broadcast {}", spell(&r)),
            };
            if got != want.trim() {
                failures.push(format!(
                    "{file}:{}\n  request {input}\n  want    {want}\n  got     {got}",
                    n + 1
                ));
            }
            records += 1;
        }
        assert!(records > 0, "{file} has no records");
        total += records;
    }
    assert!(
        failures.is_empty(),
        "{} of {total} records failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
