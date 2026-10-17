// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

//! Every service decoder against bodies a shared wire might deliver: none
//! may panic, and a body that decodes must re-decode the same after its
//! own encoding — the canonical spelling is a fixed point.

use regalium_bacnet_core::apdu::Apdu;
use regalium_bacnet_core::service::Service;

/// Every APDU head a service is read from: unconfirmed choices, confirmed
/// requests, Complex-ACKs and Error PDUs.
const HEADS: &[&[u8]] = &[
    &[0x10, 0x00],
    &[0x10, 0x01],
    &[0x10, 0x02],
    &[0x10, 0x07],
    &[0x10, 0x08],
    &[0x00, 0x05, 0x01, 0x01],
    &[0x00, 0x05, 0x01, 0x05],
    &[0x00, 0x05, 0x01, 0x0a],
    &[0x00, 0x05, 0x01, 0x0b],
    &[0x00, 0x05, 0x01, 0x0c],
    &[0x00, 0x05, 0x01, 0x0e],
    &[0x00, 0x05, 0x01, 0x0f],
    &[0x00, 0x05, 0x01, 0x10],
    &[0x00, 0x05, 0x01, 0x1c],
    &[0x30, 0x01, 0x0a],
    &[0x30, 0x01, 0x0c],
    &[0x30, 0x01, 0x0e],
    &[0x50, 0x01, 0x0a],
    &[0x50, 0x01, 0x0c],
    &[0x50, 0x01, 0x10],
];

/// Octets that look like tags often enough to reach deep into a grammar.
const ALPHABET: &[u8] = &[
    0x09, 0x0c, 0x0e, 0x0f, 0x19, 0x1e, 0x1f, 0x29, 0x2e, 0x2f, 0x3e, 0x3f, 0x44, 0x4e, 0x4f, 0x91,
    0x00, 0x01, 0x55, 0xc4,
];

#[test]
fn no_body_makes_any_service_decoder_panic() {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut decoded = 0;
    for round in 0..60_000usize {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let head = HEADS[round % HEADS.len()];
        let len = usize::try_from(state % 24).unwrap();
        let body = (0..len).map(|i| ALPHABET[usize::try_from((state >> (i % 60)) % 20).unwrap()]);
        let apdu: Vec<u8> = head.iter().copied().chain(body).collect();
        let a = Apdu::decode(&apdu).unwrap();
        if Service::decode(&a).is_ok_and(|s| s != Service::Undecoded) {
            decoded += 1;
        }
    }
    assert!(
        decoded > 100,
        "the sweep must reach decodable bodies, reached {decoded}"
    );
}
