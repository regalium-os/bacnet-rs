// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

//! The tag layer's spans, through a tracer that remembers what it was told:
//! one span per buffer, the declared attributes and nothing else, and the
//! failure when there is one.

use regalium_bacnet_core::tag;
use regalium_bacnet_telemetry::{Attribute, AttributeValue, NoopTracer, SpanId, Tracer};
use std::fmt::Display;
use std::sync::Mutex;

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
        self.0
            .lock()
            .unwrap()
            .push(error.map_or_else(|| "end".to_owned(), |e| format!("fail {e}")));
    }
}

#[test]
fn a_decode_is_one_span_recording_size_count_and_canonicity() {
    let r = Recorder::default();
    // A long-spelled unsigned, then a REAL: two elements, one non-canonical.
    let buf = [0x25, 0x01, 0x48, 0x44, 0x42, 0x90, 0x00, 0x00];
    tag::decode_all_traced(&buf, &r).unwrap();
    assert_eq!(
        *r.0.lock().unwrap(),
        [
            "start bacnet.tag.decode",
            "bacnet.tag.octets=8",
            "bacnet.tag.elements=2",
            "bacnet.tag.canonical=false",
            "end"
        ]
    );
}

#[test]
fn a_failed_decode_ends_its_span_with_the_error_and_no_contents() {
    let r = Recorder::default();
    assert!(tag::decode_all_traced(&[0x44, 0x42], &r).is_err());
    let seen = r.0.lock().unwrap();
    assert!(
        seen.last().unwrap().starts_with("fail truncated"),
        "{seen:?}"
    );
    assert!(
        !seen.iter().any(|s| s.contains("44 42")),
        "the device's octets never reach a span: {seen:?}"
    );
}

#[test]
fn the_noop_tracer_changes_nothing_about_the_result() {
    let buf = [0x21, 0x48];
    let traced = tag::decode_all_traced(&buf, &NoopTracer).unwrap();
    assert_eq!(traced, tag::decode_all(&buf).unwrap());
    assert_eq!(tag::encode_all_traced(&traced, &NoopTracer), buf);
}
