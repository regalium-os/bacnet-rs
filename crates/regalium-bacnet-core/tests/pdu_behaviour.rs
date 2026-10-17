// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

//! The NPDU and APDU beyond the corpus: what a caller builds encodes to
//! what Clause 6 and §20.1 say, no input panics, and the spans say what
//! kind of PDU it was without carrying what was in it.

use regalium_bacnet_core::apdu::{self, Apdu, Limits, Pdu, Segment, Segments};
use regalium_bacnet_core::npdu::{self, Address, Body, Destination, GLOBAL, Npdu};
use regalium_bacnet_core::{ErrorKind, tag};
use regalium_bacnet_telemetry::{Attribute, AttributeValue, NoopTracer, SpanId, Tracer};
use std::fmt::Display;
use std::sync::Mutex;

#[test]
fn a_built_npdu_writes_its_presence_bits_from_its_fields() {
    let mac = [0x0a];
    let n = Npdu {
        control: 0x04 | 0x80, // a stale message bit, which the body overrides
        destination: Some(Destination {
            address: Address {
                network: 5,
                mac: &mac,
            },
            hop_count: 255,
        }),
        source: None,
        body: Body::Apdu(&[0x10, 0x08]),
    };
    let mut out = Vec::new();
    n.encode(&mut out);
    assert_eq!(out, [0x01, 0x24, 0x00, 0x05, 0x01, 0x0a, 0xff, 0x10, 0x08]);
    assert_eq!(Npdu::decode(&out).unwrap(), Npdu { control: 0x24, ..n });
}

#[test]
fn a_global_broadcast_is_global_and_a_broadcast() {
    let a = Address {
        network: GLOBAL,
        mac: &[],
    };
    assert!(a.is_global() && a.is_broadcast());
}

#[test]
fn a_built_apdu_writes_the_segment_bit_from_its_segment() {
    let a = Apdu {
        flags: 0x04, // more follows, but no SEG bit given
        pdu: Pdu::ComplexAck {
            invoke_id: 9,
            segment: Some(Segment {
                sequence: 2,
                window: 4,
            }),
            service: 14,
            body: &[0x1f],
        },
        trailing: &[],
    };
    let mut out = Vec::new();
    a.encode(&mut out);
    assert_eq!(out, [0x3c, 0x09, 0x02, 0x04, 0x0e, 0x1f]);
    assert!(Apdu::decode(&out).unwrap().more_follows());
}

#[test]
fn limits_round_up_to_the_next_segment_count_the_standard_can_say() {
    let l = Limits::new(Segments::AtMost(10), 5);
    assert_eq!(
        (l.max_segments(), l.max_apdu()),
        (Segments::AtMost(16), Some(1476))
    );
    assert_eq!(Limits::default().max_apdu(), Some(50));
}

#[test]
fn every_layer_reports_through_one_error_type() {
    let e = Npdu::decode(&[0x02]).unwrap_err();
    assert_eq!(e.kind, ErrorKind::Version(2));
    let e: regalium_bacnet_core::Error<'_> = Apdu::decode(&[0x90]).unwrap_err();
    assert_eq!((e.kind, e.bytes), (ErrorKind::PduType(9), &[0x90][..]));
    let _same_type: regalium_bacnet_core::Error<'_> = tag::decode_all(&[0x44]).unwrap_err();
}

#[test]
fn no_input_makes_either_layer_panic_and_whatever_decodes_round_trips() {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    for _ in 0..20_000 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let len = usize::try_from(state % 24).unwrap();
        let buf: Vec<u8> = state.to_le_bytes().into_iter().cycle().take(len).collect();
        let mut npdu = buf.clone();
        npdu.insert(0, 1); // a version that passes, so the header is reached
        for (input, is_npdu) in [(&buf, false), (&npdu, true)] {
            let mut out = Vec::new();
            if is_npdu {
                if let Ok(n) = Npdu::decode(input) {
                    n.encode(&mut out);
                    assert_eq!(&out, input);
                }
            } else if let Ok(a) = Apdu::decode(input) {
                a.encode(&mut out);
                assert_eq!(&out, input);
            }
        }
    }
}

#[derive(Default)]
struct Recorder(Mutex<Vec<String>>);

impl Tracer for Recorder {
    fn start(&self, name: &'static str, _: SpanId) -> SpanId {
        self.0.lock().unwrap().push(format!("start {name}"));
        SpanId(1)
    }
    fn attribute(&self, _: SpanId, a: Attribute<'_>) {
        let v = match a.value {
            AttributeValue::Int(n) => n.to_string(),
            AttributeValue::Bool(b) => b.to_string(),
            AttributeValue::Str(s) => s.to_owned(),
        };
        self.0.lock().unwrap().push(format!("{}={v}", a.key.name()));
    }
    fn end(&self, _: SpanId, error: Option<&dyn Display>) {
        let line = error.map_or_else(|| "end".to_owned(), |e| format!("fail {e}"));
        self.0.lock().unwrap().push(line);
    }
}

#[test]
fn an_npdu_span_says_routed_and_not_where() {
    let r = Recorder::default();
    let buf = [
        0x01, 0x24, 0x00, 0x05, 0x01, 0x0a, 0xff, 0x00, 0x05, 0x07, 0x0c,
    ];
    npdu::decode_traced(&buf, &r).unwrap();
    let seen = r.0.lock().unwrap();
    assert_eq!(
        *seen,
        [
            "start bacnet.npdu.decode",
            "bacnet.pdu.octets=11",
            "bacnet.npdu.message=false",
            "bacnet.npdu.routed=true",
            "end"
        ]
    );
}

#[test]
fn an_apdu_span_carries_type_service_and_invoke_id_but_no_body() {
    let r = Recorder::default();
    let buf = [0x30, 0x07, 0x0c, 0x0c, 0x02, 0x00, 0x02, 0x57];
    apdu::decode_traced(&buf, &r).unwrap();
    let seen = r.0.lock().unwrap();
    assert_eq!(
        *seen,
        [
            "start bacnet.apdu.decode",
            "bacnet.pdu.octets=8",
            "bacnet.apdu.type=complex-ack",
            "bacnet.apdu.service_choice=12",
            "bacnet.invoke_id=7",
            "bacnet.apdu.segmented=false",
            "end"
        ]
    );
}

#[test]
fn a_failed_apdu_decode_fails_its_span_without_the_octets() {
    let r = Recorder::default();
    assert!(apdu::decode_traced(&[0x00, 0x05], &r).is_err());
    let seen = r.0.lock().unwrap();
    assert!(
        seen.last().unwrap().starts_with("fail truncated"),
        "{seen:?}"
    );
    assert!(!seen.iter().any(|s| s.contains("00 05")), "{seen:?}");
}

#[test]
fn the_noop_tracer_changes_nothing_about_the_result() {
    let buf = [0x10, 0x08];
    assert_eq!(
        apdu::decode_traced(&buf, &NoopTracer).unwrap(),
        Apdu::decode(&buf).unwrap()
    );
    let mut out = Vec::new();
    apdu::encode_traced(&Apdu::decode(&buf).unwrap(), &mut out, &NoopTracer);
    assert_eq!(out, buf);
}
