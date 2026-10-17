// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

//! The MS/TP checks against values that do not come from this code: the
//! published CRC-16/X-25 check value, and the residues the standard (and
//! Wireshark) verify frames by.

use regalium_bacnet_mstp::crc::{
    CRC32K_RESIDUE, DATA_RESIDUE, HEADER_RESIDUE, cobs_decode, cobs_encode, crc32k, data,
    data_check, header, header_check,
};
use regalium_bacnet_mstp::frame::{Frame, FrameError, kind};

#[test]
fn the_data_crc_is_crc_16_x25_by_its_published_check_value() {
    assert_eq!(u16::from_le_bytes(data_check(b"123456789")), 0x906E);
}

#[test]
fn every_check_lands_on_its_residue() {
    let head = [kind::TOKEN, 0x05, 0x10, 0x00, 0x00];
    let mut with = head.to_vec();
    with.push(header_check(&head));
    assert_eq!(with.iter().fold(0xFF, |c, b| header(*b, c)), HEADER_RESIDUE);
    let body = b"an NPDU";
    let mut with = body.to_vec();
    with.extend(data_check(body));
    assert_eq!(with.iter().fold(0xFFFF, |c, b| data(*b, c)), DATA_RESIDUE);
    let enc = cobs_encode(body);
    let crc = !enc.iter().fold(0xFFFF_FFFF, |c, b| crc32k(*b, c));
    let all = enc
        .iter()
        .chain(&crc.to_le_bytes())
        .fold(0xFFFF_FFFF, |c, b| crc32k(*b, c));
    assert_eq!(all, CRC32K_RESIDUE);
}

#[test]
fn cobs_round_trips_and_never_writes_the_mask_as_a_zero() {
    for data in [
        vec![],
        vec![0],
        vec![1, 0, 2, 0, 0, 3],
        (0..=255u8).cycle().take(600).collect::<Vec<_>>(),
    ] {
        let enc = cobs_encode(&data);
        assert!(enc.iter().all(|b| b ^ 0x55 != 0), "no zero before masking");
        assert_eq!(cobs_decode(&enc).unwrap(), data);
    }
}

#[test]
fn frames_round_trip_bare_with_data_and_extended() {
    let token = Frame::bare(kind::TOKEN, 0x05, 0x10);
    let bytes = token.encode();
    assert_eq!(&bytes[..7], &[0x55, 0xFF, 0x00, 0x05, 0x10, 0x00, 0x00]);
    assert_eq!(Frame::decode(&bytes).unwrap(), (token, 8));
    let data = Frame {
        kind: kind::DATA_NOT_EXPECTING_REPLY,
        destination: 0xFF,
        source: 3,
        data: vec![1, 0, 0x10, 8],
    };
    let bytes = data.encode();
    assert_eq!((bytes[5], bytes[6], bytes.len()), (0, 4, 8 + 4 + 2));
    assert_eq!(Frame::decode(&bytes).unwrap(), (data, 14));
    let big = Frame {
        kind: kind::EXTENDED_NOT_EXPECTING_REPLY,
        destination: 9,
        source: 3,
        data: (0..=255u8).cycle().take(1497).collect(),
    };
    let bytes = big.encode();
    assert_eq!(Frame::decode(&bytes).unwrap().0, big);
}

#[test]
fn a_corrupted_frame_is_refused_not_misread() {
    let mut bytes = Frame {
        kind: kind::DATA_EXPECTING_REPLY,
        destination: 1,
        source: 2,
        data: vec![1, 4, 0, 5],
    }
    .encode();
    bytes[9] ^= 1;
    assert_eq!(Frame::decode(&bytes), Err(FrameError::DataCrc));
    bytes[3] ^= 1;
    assert_eq!(Frame::decode(&bytes), Err(FrameError::HeaderCrc));
    assert_eq!(
        Frame::decode(&[0x55, 0xFE, 0, 0, 0, 0, 0, 0]),
        Err(FrameError::NoPreamble)
    );
    assert_eq!(
        Frame::decode(&[0x55, 0xFF, 0]),
        Err(FrameError::Truncated { needed: 8 })
    );
}
