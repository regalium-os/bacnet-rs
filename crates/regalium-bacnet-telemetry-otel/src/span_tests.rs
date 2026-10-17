// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! Seam spans, through the adapter, into an in-memory SDK exporter: the
//! name, the declared attributes under their keys, and a failure's status.

use crate::Otel;
use opentelemetry::metrics::MeterProvider;
use opentelemetry::trace::{Status, TracerProvider};
use opentelemetry::{Key, Value};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use regalium_bacnet_telemetry::{Attribute, Span, SpanId, Tracer, keys};

fn pipeline() -> (Otel, SdkTracerProvider, InMemorySpanExporter) {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    (
        Otel::from_tracer(provider.tracer("test")),
        provider,
        exporter,
    )
}

#[test]
fn a_span_reaches_the_exporter_with_its_declared_attributes() {
    let (otel, provider, exporter) = pipeline();
    let span = Span::start(&otel, "bacnet.tag.decode");
    otel.attribute(SpanId(1), Attribute::int(keys::TAG_OCTETS, 8));
    span.end();
    provider.force_flush().unwrap();
    let spans = exporter.get_finished_spans().unwrap();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].name, "bacnet.tag.decode");
    let octets = spans[0]
        .attributes
        .iter()
        .find(|kv| kv.key == Key::from("bacnet.tag.octets"));
    assert_eq!(octets.map(|kv| kv.value.clone()), Some(Value::I64(8)));
}

#[test]
fn a_failed_span_ends_with_the_error_status() {
    let (otel, provider, exporter) = pipeline();
    Span::start(&otel, "bacnet.tag.decode").fail(&"truncated at offset 0");
    provider.force_flush().unwrap();
    let spans = exporter.get_finished_spans().unwrap();
    assert!(
        matches!(&spans[0].status, Status::Error { description } if description.contains("truncated")),
        "{:?}",
        spans[0].status
    );
}

#[test]
fn without_a_tracer_nothing_is_started_or_kept() {
    let otel = Otel::from_meter(opentelemetry_sdk::metrics::SdkMeterProvider::default().meter("m"));
    assert_eq!(otel.start("bacnet.tag.decode", SpanId::NONE), SpanId::NONE);
    otel.attribute(SpanId::NONE, Attribute::int(keys::TAG_OCTETS, 1));
    otel.end(SpanId::NONE, None);
}

#[test]
fn a_child_span_belongs_to_its_parents_trace_and_names_it_as_parent() {
    let (otel, provider, exporter) = pipeline();
    let cycle = Span::start(&otel, "bacnet.e2e.cycle");
    cycle.child("bacnet.api.get_object").end();
    cycle.end();
    provider.force_flush().unwrap();
    let spans = exporter.get_finished_spans().unwrap();
    let child = spans
        .iter()
        .find(|s| s.name == "bacnet.api.get_object")
        .unwrap();
    let parent = spans.iter().find(|s| s.name == "bacnet.e2e.cycle").unwrap();
    assert_eq!(
        child.span_context.trace_id(),
        parent.span_context.trace_id()
    );
    assert_eq!(child.parent_span_id, parent.span_context.span_id());
}
