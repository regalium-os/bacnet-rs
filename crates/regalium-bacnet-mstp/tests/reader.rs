// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

//! Frames out of a noisy byte stream.

use regalium_bacnet_mstp::frame::{Frame, kind};
use regalium_bacnet_mstp::reader::Reader;

fn data() -> Frame {
    Frame {
        kind: kind::DATA_NOT_EXPECTING_REPLY,
        destination: 0xFF,
        source: 2,
        data: vec![1, 0, 0x10, 8],
    }
}

#[test]
fn noise_before_a_frame_is_skipped() {
    let mut line = vec![0x00, 0x55, 0x13, 0xFF];
    line.extend(data().encode());
    assert_eq!(Reader::new().push(&line), [data()]);
}

#[test]
fn a_frame_split_across_reads_arrives_once_whole() {
    let bytes = data().encode();
    let mut r = Reader::new();
    for b in &bytes[..bytes.len() - 1] {
        assert_eq!(r.push(&[*b]), []);
    }
    assert_eq!(r.push(&bytes[bytes.len() - 1..]), [data()]);
}

#[test]
fn a_corrupted_frame_is_counted_and_the_next_one_still_read() {
    let mut line = data().encode();
    line[9] ^= 0x40;
    line.extend(Frame::bare(kind::TOKEN, 4, 2).encode());
    let mut r = Reader::new();
    assert_eq!(r.push(&line), [Frame::bare(kind::TOKEN, 4, 2)]);
    assert_eq!(r.discarded(), 1);
}
