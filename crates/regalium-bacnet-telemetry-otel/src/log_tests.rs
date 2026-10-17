// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! Seam log lines, through the adapter, into an in-memory SDK exporter: the
//! level threshold holds, and a line written inside a span names the trace.

use crate::Otel;
use opentelemetry::logs::{AnyValue, LoggerProvider as _, Severity};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::logs::{InMemoryLogExporter, SdkLoggerProvider};
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use regalium_bacnet_telemetry::{Attribute, Level, Logger, Span, SpanId, keys};

#[test]
fn a_line_inside_a_span_carries_its_trace_and_the_threshold_holds() {
    let spans = InMemorySpanExporter::default();
    let tracer = SdkTracerProvider::builder()
        .with_simple_exporter(spans.clone())
        .build();
    let logs = InMemoryLogExporter::default();
    let logger = SdkLoggerProvider::builder()
        .with_simple_exporter(logs.clone())
        .build();
    let otel = Otel::from_tracer(tracer.tracer("t")).with_logger(logger.logger("t"), Level::Info);

    assert!(!otel.enabled(Level::Debug) && otel.enabled(Level::Warn));
    let span = Span::start(&otel, "bacnet.e2e.cycle");
    otel.log(Level::Debug, &"dropped below the threshold", span.id(), &[]);
    otel.log(
        Level::Warn,
        &"device 599 refused",
        span.id(),
        &[Attribute::int(keys::DEVICE_INSTANCE, 599)],
    );
    otel.log(Level::Info, &"outside any span", SpanId::NONE, &[]);
    span.end();
    logger.force_flush().unwrap();

    let records = logs.get_emitted_logs().unwrap();
    assert_eq!(records.len(), 2, "the debug line is not kept");
    let warn = &records[0].record;
    assert_eq!(warn.severity_number(), Some(Severity::Warn));
    assert_eq!(
        warn.body(),
        Some(&AnyValue::from("device 599 refused".to_owned()))
    );
    let attr = |name: &str| {
        warn.attributes_iter()
            .find(|(k, _)| k.as_str() == name)
            .map(|(_, v)| v.clone())
    };
    assert!(
        attr("trace_id").is_some() && attr("span_id").is_some(),
        "correlated with its span"
    );
    assert_eq!(
        attr("bacnet.device.instance"),
        Some(AnyValue::from("599".to_owned()))
    );
    let plain = &records[1].record;
    assert!(
        plain
            .attributes_iter()
            .all(|(k, _)| k.as_str() != "trace_id"),
        "no span, no trace"
    );
}
