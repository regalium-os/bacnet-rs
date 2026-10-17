// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The transaction state machine fed whatever arrives, in any order, at any
//! time: it never panics, and every transaction ends once its deadlines
//! are followed.

#![no_main]

use libfuzzer_sys::fuzz_target;
use regalium_bacnet::apdu::Apdu;
use regalium_bacnet::tsm::{Config, Remote, Request, Step, Tsm};
use std::time::Duration;

fuzz_target!(|data: &[u8]| {
    let mut tsm = Tsm::new(Config::default().retries(1));
    let remote = Remote { max_apdu: 50, segmented_requests: true, ..Remote::default() };
    let mut now = Duration::ZERO;
    // Each chunk is an operation: its first octet says which.
    for chunk in data.chunks(12) {
        let Some((&op, rest)) = chunk.split_first() else { continue };
        now += Duration::from_millis(u64::from(op) * 37);
        match op % 3 {
            0 => {
                let _ = tsm.request(u8::from(op & 1 == 0), Request::new(12, rest.repeat(8)).remote(remote), now);
            }
            1 => {
                if let Ok(apdu) = Apdu::decode(rest) {
                    tsm.receive(op & 1, &apdu, now);
                }
            }
            _ => {}
        }
        for _ in 0..1024 {
            if matches!(tsm.poll(now), Step::Idle { .. }) {
                break;
            }
        }
    }
    // Following each deadline as a runtime would, every transaction ends:
    // retried until its retries are spent, then abandoned.
    let mut at = now;
    for _ in 0..100_000 {
        match tsm.poll(at) {
            Step::Idle { wake_at: None } => return,
            Step::Idle { wake_at: Some(w) } => at = w.max(at),
            _ => {}
        }
    }
    panic!("transactions still open after following every deadline");
});
