// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! BACnet/IPv6's gate: every record of the BVLL-IPv6 corpora decodes to what
//! the record says, and re-encodes to exactly its octets.

use regalium_bacnet_ipv6::bvll::{Bvll6, Bvll6ErrorKind, Function6};

/// Every corpus. `just fixtures` fails when one on disk is missing here.
const CORPORA: &[(&str, &str)] = &[
    (
        "messages.hex",
        include_str!("../fixtures/bvll6/messages.hex"),
    ),
    (
        "malformed-bvll6.hex",
        include_str!("../fixtures/bvll6/malformed-bvll6.hex"),
    ),
];

fn hex(b: &[u8]) -> String {
    use std::fmt::Write;
    b.iter().fold(String::new(), |mut out, x| {
        let _ = write!(out, "{x:02x}");
        out
    })
}

fn meaning(b: &Bvll6<'_>) -> String {
    let from = b.source.map(|s| format!(" from {s}")).unwrap_or_default();
    match b.function {
        Function6::Result(c) => format!("result{from} {c:#06x}"),
        Function6::OriginalUnicast { destination, npdu } => {
            format!("original-unicast{from} to {destination} npdu {}", hex(npdu))
        }
        Function6::OriginalBroadcast(n) => format!("original-broadcast{from} npdu {}", hex(n)),
        Function6::AddressResolution(t) => format!("address-resolution{from} for {t}"),
        Function6::ForwardedAddressResolution { target, origin } => {
            format!("forwarded-address-resolution{from} for {target} origin {origin}")
        }
        Function6::AddressResolutionAck(d) => format!("address-resolution-ack{from} to {d}"),
        Function6::VirtualAddressResolution => format!("virtual-address-resolution{from}"),
        Function6::VirtualAddressResolutionAck(d) => {
            format!("virtual-address-resolution-ack{from} to {d}")
        }
        Function6::Forwarded { origin, npdu } => {
            format!("forwarded{from} origin {origin} npdu {}", hex(npdu))
        }
        Function6::RegisterForeignDevice { ttl } => {
            format!("register-foreign-device{from} ttl {ttl}")
        }
        Function6::DeleteFdtEntry(a) => format!("delete-fdt-entry{from} {a}"),
        Function6::SecureBvll(d) => format!("secure-bvll data {}", hex(d)),
        Function6::DistributeBroadcast(n) => format!("distribute-broadcast{from} npdu {}", hex(n)),
        Function6::Unknown { function, data } => {
            format!("unknown {function}{from} data {}", hex(data))
        }
    }
}

fn decode(octets: &[u8]) -> String {
    match Bvll6::decode(octets) {
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
            Bvll6ErrorKind::Truncated { needed, available } => {
                format!("error truncated {needed}/{available}")
            }
            Bvll6ErrorKind::NotBacnetIpv6(t) => format!("error not-bacnet-ipv6 {t}"),
            Bvll6ErrorKind::WrongLength { function, len } => {
                format!("error wrong-length {function}/{len}")
            }
            other => format!("error {other:?}"),
        },
    }
}

#[test]
fn every_bvll6_fixture_record_decodes_to_its_meaning_and_round_trips() {
    assert!(CORPORA.len() >= 2, "a walker that finds nothing must fail");
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
                .map(|b| u8::from_str_radix(b, 16).unwrap())
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
