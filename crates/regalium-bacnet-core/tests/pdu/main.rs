// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Phase 4's gate: every record of the NPDU and APDU corpora decodes to
//! what the record says, and re-encodes to exactly its octets.
//!
//! An NPDU record is a whole datagram after its datalink header, and its
//! meaning spells the APDU inside it too, so one record proves both layers
//! agree. An APDU record is the APDU alone. The corpora are compiled in, as
//! the tag corpus is: the purity gate refuses file access in the core.

mod network;
mod render;

use regalium_bacnet_core::apdu::Apdu;
use regalium_bacnet_core::npdu::{Body, Npdu};

/// Every corpus: its file name, and whether it holds NPDUs or bare APDUs.
/// `just fixtures` fails when a `.hex` file on disk is missing here.
const CORPORA: &[(&str, bool, &str)] = &[
    (
        "local.hex",
        true,
        include_str!("../../fixtures/npdu/local.hex"),
    ),
    (
        "routed.hex",
        true,
        include_str!("../../fixtures/npdu/routed.hex"),
    ),
    (
        "network.hex",
        true,
        include_str!("../../fixtures/npdu/network.hex"),
    ),
    (
        "simulator.hex",
        true,
        include_str!("../../fixtures/npdu/simulator.hex"),
    ),
    (
        "malformed-npdu.hex",
        true,
        include_str!("../../fixtures/npdu/malformed-npdu.hex"),
    ),
    (
        "requests.hex",
        false,
        include_str!("../../fixtures/apdu/requests.hex"),
    ),
    (
        "segments.hex",
        false,
        include_str!("../../fixtures/apdu/segments.hex"),
    ),
    (
        "malformed-apdu.hex",
        false,
        include_str!("../../fixtures/apdu/malformed-apdu.hex"),
    ),
];

fn octets(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("hex octet"))
        .collect()
}

/// Decodes, checks the round trip of every layer that decoded, and renders.
fn decode(octets: &[u8], npdu: bool) -> String {
    if !npdu {
        return match Apdu::decode(octets) {
            Ok(a) => {
                let mut again = Vec::new();
                a.encode(&mut again);
                assert_eq!(again, octets, "APDU re-encoding is not lossless");
                render::apdu(&a)
            }
            Err(e) => render::error(&e),
        };
    }
    match Npdu::decode(octets) {
        Ok(n) => {
            let mut again = Vec::new();
            n.encode(&mut again);
            assert_eq!(again, octets, "NPDU re-encoding is not lossless");
            if let Body::Apdu(inner) = n.body
                && let Ok(a) = Apdu::decode(inner)
            {
                let mut again = Vec::new();
                a.encode(&mut again);
                assert_eq!(again, inner, "APDU re-encoding is not lossless");
            }
            network::npdu(&n)
        }
        Err(e) => render::error(&e),
    }
}

#[test]
fn every_npdu_and_apdu_fixture_record_decodes_to_its_meaning_and_round_trips() {
    assert!(CORPORA.len() >= 8, "a walker that finds nothing must fail");
    let mut failures = Vec::new();
    let mut total = 0;
    for (file, npdu, text) in CORPORA {
        let mut records = 0;
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (input, want) = line
                .split_once(" => ")
                .unwrap_or_else(|| panic!("{file}:{}: no ' => '", n + 1));
            let got = decode(&octets(input), *npdu);
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
