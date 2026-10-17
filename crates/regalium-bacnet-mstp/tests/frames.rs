// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! MS/TP's gate: every record of the frame corpora decodes to what the
//! record says, and a frame that decodes re-encodes to exactly its octets.

use regalium_bacnet_mstp::frame::{Frame, FrameError, kind};
use std::fmt::Write;

/// Every corpus. `just fixtures` fails when one on disk is missing here.
const CORPORA: &[(&str, &str)] = &[
    ("frames.hex", include_str!("../fixtures/mstp/frames.hex")),
    (
        "malformed-frames.hex",
        include_str!("../fixtures/mstp/malformed-frames.hex"),
    ),
];

fn name(k: u8) -> String {
    match k {
        kind::TOKEN => "token".into(),
        kind::POLL_FOR_MASTER => "poll-for-master".into(),
        kind::REPLY_TO_POLL_FOR_MASTER => "reply-to-poll-for-master".into(),
        kind::TEST_REQUEST => "test-request".into(),
        kind::TEST_RESPONSE => "test-response".into(),
        kind::DATA_EXPECTING_REPLY => "data-expecting-reply".into(),
        kind::DATA_NOT_EXPECTING_REPLY => "data-not-expecting-reply".into(),
        kind::REPLY_POSTPONED => "reply-postponed".into(),
        kind::EXTENDED_EXPECTING_REPLY => "extended-expecting-reply".into(),
        kind::EXTENDED_NOT_EXPECTING_REPLY => "extended-not-expecting-reply".into(),
        other => format!("type-{other}"),
    }
}

fn decode(octets: &[u8]) -> String {
    match Frame::decode(octets) {
        Ok((f, used)) => {
            assert_eq!(used, octets.len(), "a record is one whole frame");
            assert_eq!(f.encode(), octets, "re-encoding is not lossless");
            let data = f.data.iter().fold(String::new(), |mut out, b| {
                let _ = write!(out, "{b:02x}");
                out
            });
            let data = if data.is_empty() {
                String::new()
            } else {
                format!(" data {data}")
            };
            format!(
                "{} to {} from {}{data}",
                name(f.kind),
                f.destination,
                f.source
            )
        }
        Err(FrameError::Truncated { needed }) => format!("error truncated {needed}"),
        Err(FrameError::NoPreamble) => "error no-preamble".into(),
        Err(FrameError::HeaderCrc) => "error header-crc".into(),
        Err(FrameError::DataCrc) => "error data-crc".into(),
        Err(FrameError::Encoding) => "error encoding".into(),
    }
}

#[test]
fn every_record_of_every_corpus_decodes_as_it_says() {
    let mut records = 0;
    for (file, text) in CORPORA {
        let mut here = 0;
        for line in text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        {
            let (octets, meaning) = line.split_once(" => ").expect("octets => meaning");
            let octets: Vec<u8> = octets
                .split_whitespace()
                .map(|h| u8::from_str_radix(h, 16).expect("hex"))
                .collect();
            assert_eq!(decode(&octets), meaning, "{file}: {line}");
            here += 1;
        }
        assert!(here > 0, "{file} has no records");
        records += here;
    }
    assert!(records >= 17, "only {records} records");
}
