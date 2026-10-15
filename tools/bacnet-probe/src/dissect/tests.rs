// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

use super::frame;

fn texts(datagram: &[u8]) -> Vec<String> {
    frame(datagram).into_iter().map(|f| f.text).collect()
}

#[test]
fn a_read_property_request_names_every_field() {
    // Unicast, expecting reply; Confirmed-Request invoke 1, ReadProperty
    // device:599 object-name.
    let t = texts(&[
        0x81, 0x0A, 0x00, 0x11, 0x01, 0x04, 0x00, 0x05, 0x01, 0x0C, 0x0C, 0x02, 0x00, 0x02, 0x57,
        0x19, 0x4D,
    ]);
    assert_eq!(t.len(), 11, "{t:#?}");
    assert!(t[1].contains("Original-Unicast-NPDU"));
    assert!(t[4].contains("expecting reply"));
    assert!(t[6].contains("max APDU accepted 1476"));
    assert!(t[8].contains("readProperty"));
    assert!(t[9].ends_with("object-identifier = device:599"), "{}", t[9]);
    assert!(
        t[10].ends_with("property-identifier = object-name (77)"),
        "{}",
        t[10]
    );
}

#[test]
fn a_read_property_ack_interprets_the_value_by_its_property() {
    // Complex-ACK: analog-input:1 units = 62 (degrees-celsius).
    let t = texts(&[
        0x81, 0x0A, 0x00, 0x14, 0x01, 0x00, 0x30, 0x01, 0x0C, 0x0C, 0x00, 0x00, 0x00, 0x01, 0x19,
        0x75, 0x3E, 0x91, 0x3E, 0x3F,
    ]);
    assert!(t.iter().any(|l| l.contains("property-value")), "{t:#?}");
    assert!(
        t.iter().any(|l| l.contains("degrees-celsius (62)")),
        "{t:#?}"
    );
}

#[test]
fn an_i_am_labels_its_four_parameters() {
    let t = texts(&[
        0x81, 0x0B, 0x00, 0x14, 0x01, 0x00, 0x10, 0x00, 0xC4, 0x02, 0x00, 0x02, 0x57, 0x22, 0x05,
        0xC4, 0x91, 0x03, 0x21, 0x07,
    ]);
    let joined = t.join("\n");
    for want in [
        "i-Am",
        "i-am-device-identifier = device:599",
        "max-apdu-length-accepted = 1476",
        "segmentation-supported = no-segmentation (3)",
        "vendor-id = 7",
    ] {
        assert!(joined.contains(want), "missing {want:?} in\n{joined}");
    }
}

#[test]
fn a_truncated_frame_shows_its_leftover_octets_instead_of_failing() {
    let fields = frame(&[
        0x81, 0x0A, 0x00, 0x0B, 0x01, 0x00, 0x30, 0x01, 0x0C, 0x0C, 0x00,
    ]);
    let last = fields.last().unwrap();
    assert!(last.text.contains("not dissected"), "{}", last.text);
    assert_eq!(last.bytes, [0x0C, 0x00]);
}

#[test]
fn a_summary_names_the_pdu_the_service_and_its_target() {
    let request = [
        0x81, 0x0A, 0x00, 0x11, 0x01, 0x04, 0x00, 0x05, 0x01, 0x0C, 0x0C, 0x02, 0x00, 0x02, 0x57,
        0x19, 0x4D,
    ];
    assert_eq!(
        super::summary(&request),
        "Confirmed-Request readProperty device:599 object-name (77)"
    );
}
