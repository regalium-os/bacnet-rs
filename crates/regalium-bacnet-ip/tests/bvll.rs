// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Phase 7's gate: every record of the BVLL corpora decodes to what the
//! record says, and re-encodes to exactly its octets.

use regalium_bacnet_ip::bvll::{Bvll, BvllErrorKind, Function};

/// Every corpus. `just fixtures` fails when one on disk is missing here.
const CORPORA: &[(&str, &str)] = &[
    ("npdus.hex", include_str!("../fixtures/bvll/npdus.hex")),
    (
        "management.hex",
        include_str!("../fixtures/bvll/management.hex"),
    ),
    (
        "malformed-bvll.hex",
        include_str!("../fixtures/bvll/malformed-bvll.hex"),
    ),
];

fn hex(b: &[u8]) -> String {
    use std::fmt::Write;
    b.iter().fold(String::new(), |mut out, x| {
        let _ = write!(out, "{x:02x}");
        out
    })
}

fn data(d: &[u8]) -> String {
    if d.is_empty() {
        "(empty)".into()
    } else {
        hex(d)
    }
}

fn meaning(b: &Bvll<'_>) -> String {
    match b.function {
        Function::Result(code) => format!("result {code:#06x}"),
        Function::WriteBdt(t) | Function::ReadBdtAck(t) => {
            let name = if matches!(b.function, Function::WriteBdt(_)) {
                "write-bdt"
            } else {
                "read-bdt-ack"
            };
            let all: Vec<_> = t
                .iter()
                .map(|e| format!("{}/{}", e.address, e.mask))
                .collect();
            format!(
                "{name} {}",
                if all.is_empty() {
                    "(empty)".into()
                } else {
                    all.join(", ")
                }
            )
        }
        Function::ReadBdt => "read-bdt".into(),
        Function::Forwarded { origin, npdu } => {
            format!("forwarded from {origin} npdu {}", data(npdu))
        }
        Function::RegisterForeignDevice { ttl } => format!("register-foreign-device ttl {ttl}"),
        Function::ReadFdt => "read-fdt".into(),
        Function::ReadFdtAck(t) => {
            let all: Vec<_> = t
                .iter()
                .map(|e| format!("{} ttl {} remaining {}", e.address, e.ttl, e.remaining))
                .collect();
            format!("read-fdt-ack {}", all.join(", "))
        }
        Function::DeleteFdtEntry(a) => format!("delete-fdt-entry {a}"),
        Function::DistributeBroadcast(n) => format!("distribute-broadcast npdu {}", data(n)),
        Function::OriginalUnicast(n) => format!("original-unicast npdu {}", data(n)),
        Function::OriginalBroadcast(n) => format!("original-broadcast npdu {}", data(n)),
        Function::SecureBvll(d) => format!("secure-bvll data {}", data(d)),
        Function::Unknown { function, data: d } => format!("unknown {function} data {}", data(d)),
    }
}

fn decode(octets: &[u8]) -> String {
    match Bvll::decode(octets) {
        Ok(b) => {
            let mut again = Vec::new();
            b.encode(&mut again);
            assert_eq!(again, octets, "re-encoding is not lossless");
            let text = meaning(&b);
            if usize::from(b.length) == octets.len() {
                text
            } else {
                format!("{text} (length {}, datagram {})", b.length, octets.len())
            }
        }
        Err(e) => match e.kind {
            BvllErrorKind::Truncated { needed, available } => {
                format!("error truncated {needed}/{available}")
            }
            BvllErrorKind::NotBacnetIp(t) => format!("error not-bacnet-ip {t}"),
            BvllErrorKind::WrongLength { function, len } => {
                format!("error wrong-length {function}/{len}")
            }
            other => format!("error {other:?}"),
        },
    }
}

#[test]
fn every_bvll_fixture_record_decodes_to_its_meaning_and_round_trips() {
    assert!(CORPORA.len() >= 3, "a walker that finds nothing must fail");
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
            let octets: Vec<u8> = input
                .split_whitespace()
                .map(|b| u8::from_str_radix(b, 16).expect("hex octet"))
                .collect();
            let got = decode(&octets);
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
