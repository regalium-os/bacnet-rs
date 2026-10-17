// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation
)]

//! The transaction state machine beyond the corpus: what is too long to
//! spell as a scenario: 256 open requests, 300 segments, every APDU in
//! every state. The spans are in `tsm_spans.rs`.

use regalium_bacnet_core::apdu::{Apdu, Pdu, Segments};
use regalium_bacnet_core::tsm::{Config, Received, Refused, Remote, Reply, Request, Step, Tsm};
use std::time::Duration;

const T0: Duration = Duration::ZERO;

fn drain(tsm: &mut Tsm<u8>, now: Duration) -> Vec<Step<u8>> {
    let mut out = Vec::new();
    loop {
        match tsm.poll(now) {
            Step::Idle { .. } => return out,
            step => out.push(step),
        }
    }
}

fn sent(steps: &[Step<u8>]) -> Vec<Vec<u8>> {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Transmit { apdu, .. } => Some(apdu.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_257th_open_request_is_refused_and_an_answer_frees_an_id() {
    let mut tsm = Tsm::new(Config::default());
    for i in 0..=255u8 {
        assert_eq!(tsm.request(1, Request::new(12, vec![]), T0), Ok(i));
    }
    assert_eq!(
        tsm.request(2, Request::new(12, vec![]), T0),
        Err(Refused::NoInvokeId)
    );
    let _ = drain(&mut tsm, T0);
    let ack = Apdu::decode(&[0x20, 0x07, 0x0c]).unwrap();
    assert_eq!(tsm.receive(1, &ack, T0), Received::Matched);
    assert_eq!(tsm.request(2, Request::new(12, vec![]), T0), Ok(7));
    assert_eq!(tsm.open(), 256);
}

#[test]
fn a_request_of_300_segments_wraps_its_sequence_numbers_and_completes() {
    let mut tsm = Tsm::new(Config::default().window(127));
    let remote = Remote {
        max_apdu: 10,
        segmented_requests: true,
        max_segments: Segments::MoreThan64,
    };
    let body: Vec<u8> = (0..1200u32).map(|i| (i % 251) as u8).collect();
    tsm.request(4, Request::new(15, body.clone()).remote(remote), T0)
        .unwrap();
    let mut got = Vec::new();
    for _ in 0..400 {
        let segments = sent(&drain(&mut tsm, T0));
        let Some(last) = segments.last() else { break };
        for s in &segments {
            got.extend_from_slice(&s[6..]);
        }
        let seq = Apdu::decode(last).map(|a| match a.pdu {
            Pdu::ConfirmedRequest {
                segment: Some(s), ..
            } => s.sequence,
            _ => panic!("not a segment"),
        });
        let ack = [0x41, 0x00, seq.unwrap(), 127];
        tsm.receive(4, &Apdu::decode(&ack).unwrap(), T0);
    }
    assert_eq!(
        got, body,
        "every segment sent once, in order, across the wrap"
    );
    let done = Apdu::decode(&[0x20, 0x00, 0x0f]).unwrap();
    tsm.receive(4, &done, T0);
    assert!(matches!(
        drain(&mut tsm, T0)[..],
        [Step::Complete {
            reply: Reply::Simple,
            ..
        }]
    ));
}

#[test]
fn an_answer_of_300_segments_reassembles_across_the_wrap() {
    let mut tsm = Tsm::new(Config::default().window(127));
    tsm.request(4, Request::new(14, vec![]), T0).unwrap();
    let _ = drain(&mut tsm, T0);
    let mut acks = 0;
    for i in 0..300usize {
        let flags = if i + 1 < 300 { 0x3c } else { 0x38 };
        let seg = [flags, 0x00, (i % 256) as u8, 127, 0x0e, (i % 256) as u8];
        tsm.receive(4, &Apdu::decode(&seg).unwrap(), T0);
        let steps = drain(&mut tsm, T0);
        acks += sent(&steps).len();
        if let Some(Step::Complete {
            reply: Reply::Complex(all),
            ..
        }) = steps.last()
        {
            let want: Vec<u8> = (0..300usize).map(|i| (i % 256) as u8).collect();
            assert_eq!(*all, want);
            assert_eq!(
                acks,
                1 + 299 / 127 + 1,
                "segment 0, each full window, the last"
            );
            return;
        }
    }
    panic!("never completed");
}

#[test]
fn the_default_config_is_the_standards_recommendation() {
    let mut tsm: Tsm<u8> = Tsm::new(Config::default());
    tsm.request(1, Request::new(12, vec![]), T0).unwrap();
    let _ = drain(&mut tsm, T0);
    assert_eq!(
        tsm.poll(T0),
        Step::Idle {
            wake_at: Some(Duration::from_secs(3))
        }
    );
    for s in 1..=3 {
        let steps = drain(&mut tsm, Duration::from_secs(3 * s));
        assert!(matches!(steps[..], [Step::Transmit { .. }]), "retry {s}");
    }
    assert!(matches!(
        drain(&mut tsm, Duration::from_secs(12))[..],
        [Step::Abandon { .. }]
    ));
}

#[test]
fn no_apdu_in_any_state_makes_the_machine_panic() {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut tsm = Tsm::new(Config::default().window(3).max_reassembly(64));
    let remote = Remote {
        max_apdu: 12,
        segmented_requests: true,
        max_segments: Segments::Unspecified,
    };
    for round in 0..20_000u64 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let now = Duration::from_millis(round * 37);
        if tsm.open() < 4 {
            let _ = tsm.request(
                1,
                Request::new(15, vec![0; (state % 40) as usize]).remote(remote),
                now,
            );
        }
        let mut buf = state.to_le_bytes().to_vec();
        buf[1] %= 4; // land on an open invoke id often enough to matter
        if let Ok(apdu) = Apdu::decode(&buf[..(state % 9) as usize]) {
            tsm.receive(1, &apdu, now);
        }
        let _ = drain(&mut tsm, now);
    }
}
