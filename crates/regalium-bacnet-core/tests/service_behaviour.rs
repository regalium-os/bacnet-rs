// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! The services beyond the corpus: a `ReadProperty` driven through the
//! transaction state machine end to end, names that round-trip, no body
//! that panics, and spans that name what was read but not its value.

use regalium_bacnet_core::apdu::Apdu;
use regalium_bacnet_core::enumerated::{
    Segmentation, object_type, parse_object_type, parse_property, property,
};
use regalium_bacnet_core::primitive::{ObjectIdentifier, Primitive};
use regalium_bacnet_core::service::{self, IAm, ReadProperty, ReadPropertyAck, Service, confirmed};
use regalium_bacnet_core::tsm::{Config, Remote, Reply, Request, Step, Tsm};
use regalium_bacnet_telemetry::{Attribute, AttributeValue, SpanId, Tracer};
use std::fmt::Display;
use std::sync::Mutex;
use std::time::Duration;

const AI1: ObjectIdentifier = ObjectIdentifier {
    object_type: 0,
    instance: 1,
};

#[test]
fn a_read_property_goes_out_through_the_tsm_and_its_answer_comes_back_decoded() {
    let i_am = IAm::decode(&[
        0xc4, 0x02, 0, 0x02, 0x57, 0x22, 0x05, 0xc4, 0x91, 0x03, 0x21, 0x05,
    ])
    .unwrap();
    let remote = Remote {
        max_apdu: u16::try_from(i_am.max_apdu).unwrap(),
        segmented_requests: i_am.segmentation.receives(),
        ..Remote::default()
    };
    let mut body = Vec::new();
    ReadProperty {
        object: AI1,
        property: 85,
        index: None,
    }
    .encode(&mut body);
    let mut tsm = Tsm::new(Config::default());
    let invoke = tsm
        .request(
            599u32,
            Request::new(confirmed::READ_PROPERTY, body).remote(remote),
            Duration::ZERO,
        )
        .unwrap();
    let Step::Transmit { apdu, .. } = tsm.poll(Duration::ZERO) else {
        panic!("nothing sent")
    };
    let sent = Apdu::decode(&apdu).unwrap();
    assert_eq!(
        Service::decode(&sent).unwrap(),
        Service::ReadProperty(ReadProperty {
            object: AI1,
            property: 85,
            index: None
        })
    );

    let answer = [
        0x30, invoke, 0x0c, 0x0c, 0, 0, 0, 1, 0x19, 0x55, 0x3e, 0x44, 0x42, 0xc8, 0, 0, 0x3f,
    ];
    tsm.receive(
        599,
        &Apdu::decode(&answer).unwrap(),
        Duration::from_millis(40),
    );
    let Step::Complete {
        reply: Reply::Complex(result),
        ..
    } = tsm.poll(Duration::from_millis(40))
    else {
        panic!("no answer")
    };
    let ack = ReadPropertyAck::decode(&result).unwrap();
    assert_eq!(
        (ack.read.object, ack.primitive()),
        (AI1, Some(Primitive::Real(100.0)))
    );
}

#[test]
fn every_name_parses_back_to_its_number_and_unnamed_numbers_survive() {
    for t in 0..1024u16 {
        let name = object_type(t).map_or_else(|| format!("type-{t}"), str::to_owned);
        assert_eq!(parse_object_type(&name), Some(t), "{name}");
    }
    for p in 0..1000u32 {
        let name = property(p).map_or_else(|| format!("property-{p}"), str::to_owned);
        assert_eq!(parse_property(&name), Some(p), "{name}");
    }
    assert_eq!(
        (object_type(59), property(85)),
        (Some("lift"), Some("present-value"))
    );
    assert_eq!(Segmentation::from_u32(9).to_u32(), 9);
}

#[test]
fn no_service_body_makes_a_decoder_panic() {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    for _ in 0..20_000 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let len = usize::try_from(state % 20).unwrap();
        let mut apdu = vec![0x30, 0x01, 0x0c];
        apdu.extend(state.to_le_bytes().into_iter().cycle().take(len));
        let _ = Service::decode(&Apdu::decode(&apdu).unwrap());
        for head in [[0x10u8, 0x08], [0x10, 0x00]] {
            let mut u = head.to_vec();
            u.extend(state.to_be_bytes().into_iter().cycle().take(len));
            let _ = Service::decode(&Apdu::decode(&u).unwrap());
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
fn a_read_property_ack_span_names_what_was_read_and_not_the_value() {
    let r = Recorder::default();
    let ack = [
        0x30, 0x01, 0x0c, 0x0c, 0, 0, 0, 1, 0x19, 0x55, 0x3e, 0x44, 0x42, 0xc8, 0, 0, 0x3f,
    ];
    service::decode_traced(&Apdu::decode(&ack).unwrap(), &r).unwrap();
    assert_eq!(
        *r.0.lock().unwrap(),
        [
            "start bacnet.service.read_property_ack",
            "bacnet.service=read-property-ack",
            "bacnet.object.type=analog-input",
            "bacnet.object.instance=1",
            "bacnet.property.identifier=present-value",
            "end",
        ]
    );
}

#[test]
fn a_pdu_with_no_service_opens_no_span() {
    let r = Recorder::default();
    service::decode_traced(&Apdu::decode(&[0x20, 0x01, 0x0f]).unwrap(), &r).unwrap();
    assert!(r.0.lock().unwrap().is_empty());
}
