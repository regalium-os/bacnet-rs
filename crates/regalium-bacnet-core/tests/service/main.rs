// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Phase 6's gate: every record of the service corpora decodes to what the
//! record says, and a decoded service re-encodes to the body it came from —
//! or the record says `(non-canonical)`, because the device spelled a field
//! the long way and only the tag layer keeps that spelling.
//!
//! A record is a whole APDU, so the corpus also proves which APDU type and
//! service choice each service is read from.

mod events;
mod render;
mod services;

use regalium_bacnet_core::apdu::{Apdu, Pdu};
use regalium_bacnet_core::service::{ObjectBody, Service};

/// Every corpus. `just fixtures` fails when one on disk is missing here.
const CORPORA: &[(&str, &str)] = &[
    (
        "discovery.hex",
        include_str!("../../fixtures/service/discovery.hex"),
    ),
    (
        "read-property.hex",
        include_str!("../../fixtures/service/read-property.hex"),
    ),
    (
        "simulator-services.hex",
        include_str!("../../fixtures/service/simulator-services.hex"),
    ),
    (
        "who-has.hex",
        include_str!("../../fixtures/service/who-has.hex"),
    ),
    (
        "write.hex",
        include_str!("../../fixtures/service/write.hex"),
    ),
    ("rpm.hex", include_str!("../../fixtures/service/rpm.hex")),
    ("cov.hex", include_str!("../../fixtures/service/cov.hex")),
    (
        "objects.hex",
        include_str!("../../fixtures/service/objects.hex"),
    ),
    (
        "events.hex",
        include_str!("../../fixtures/service/events.hex"),
    ),
    (
        "malformed-service.hex",
        include_str!("../../fixtures/service/malformed-service.hex"),
    ),
];

fn octets(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("hex octet"))
        .collect()
}

/// The service body an APDU carries, for the round trip.
fn body<'a>(apdu: &Apdu<'a>) -> &'a [u8] {
    match apdu.pdu {
        Pdu::ConfirmedRequest { body, .. }
        | Pdu::UnconfirmedRequest { body, .. }
        | Pdu::ComplexAck { body, .. }
        | Pdu::Error { body, .. } => body,
        _ => &[],
    }
}

fn encode(s: &Service<'_>) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    match s {
        Service::WhoIs(w) => w.encode(&mut out),
        Service::IAm(i) => i.encode(&mut out),
        Service::WhoHas(w) => w.encode(&mut out),
        Service::IHave(i) => i.encode(&mut out),
        Service::ReadProperty(r) => r.encode(&mut out),
        Service::ReadPropertyAck(a) => a.encode(&mut out),
        Service::ReadPropertyMultiple(r) => r.encode(&mut out),
        Service::ReadPropertyMultipleAck(a) => a.encode(&mut out),
        Service::WriteProperty(w) => w.encode(&mut out),
        Service::WritePropertyMultiple(w) => w.encode(&mut out),
        Service::SubscribeCov(c) => c.encode(&mut out),
        Service::SubscribeCovProperty(c) => c.encode(&mut out),
        Service::CovNotification { notification, .. } => notification.encode(&mut out),
        Service::EventNotification { notification, .. } => notification.encode(&mut out),
        Service::CreateObject(c) => c.encode(&mut out),
        Service::CreateObjectAck(o) | Service::DeleteObject(o) => ObjectBody(*o).encode(&mut out),
        Service::Error { error, .. } => error.encode(&mut out),
        Service::CreateObjectError(e) => e.encode(&mut out),
        Service::WriteMultipleError(e) => e.encode(&mut out),
        _ => return None,
    }
    Some(out)
}

fn decode(octets: &[u8]) -> String {
    let apdu = Apdu::decode(octets).expect("a service record's APDU decodes");
    match Service::decode(&apdu) {
        Err(e) => render::error(&e),
        Ok(s) => {
            let text = services::service(&s);
            match encode(&s) {
                Some(again) if again != body(&apdu) => format!("{text} (non-canonical)"),
                _ => text,
            }
        }
    }
}

#[test]
fn every_service_fixture_record_decodes_to_its_meaning_and_round_trips() {
    assert!(CORPORA.len() >= 10, "a walker that finds nothing must fail");
    let mut failures = Vec::new();
    let mut total = 0;
    for (file, text) in CORPORA {
        let mut records = 0;
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (input, want) = line
                .split_once(" => ")
                .unwrap_or_else(|| panic!("{file}:{}: no ' => '", n + 1));
            let got = decode(&octets(input));
            if got != want.trim() {
                failures.push(format!(
                    "{file}:{}\n  octets {input}\n  want   {want}\n  got    {got}",
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
