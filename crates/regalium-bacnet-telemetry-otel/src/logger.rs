// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The seam's [`Logger`], over the SDK's OpenTelemetry log pipeline.
//!
//! Each line goes to OTLP only — never the console, which under a terminal
//! UI is the screen. A line written inside a span carries that span's
//! `trace_id` and `span_id`, so a log query leads straight to the trace.

use super::Otel;
use opentelemetry::KeyValue;
use opentelemetry::logs::Severity;
use regalium_bacnet_telemetry::{Attribute, AttributeValue, Level, Logger, SpanId};
use std::fmt::Display;
use std::sync::Arc;
use telemetry::logging::OtelLogger;

/// Where lines go, and the least severe one kept.
pub(crate) struct Logs {
    sink: Option<Arc<OtelLogger>>,
    min: Level,
}

impl Logs {
    pub(crate) fn new(sink: Option<Arc<OtelLogger>>, min: Level) -> Self {
        Self { sink, min }
    }
}

impl Default for Logs {
    fn default() -> Self {
        Self::new(None, Level::Info)
    }
}

impl Logger for Otel {
    fn enabled(&self, level: Level) -> bool {
        self.logs.sink.is_some() && level >= self.logs.min
    }

    fn log(&self, level: Level, message: &dyn Display, span: SpanId, attributes: &[Attribute<'_>]) {
        let Some(sink) = self.logs.sink.as_ref().filter(|_| self.enabled(level)) else {
            return;
        };
        let mut kv: Vec<KeyValue> = attributes
            .iter()
            .map(|a| match a.value {
                AttributeValue::Str(v) => KeyValue::new(a.key.name(), v.to_owned()),
                AttributeValue::Int(v) => KeyValue::new(a.key.name(), v),
                AttributeValue::Bool(v) => KeyValue::new(a.key.name(), v),
            })
            .collect();
        kv.push(KeyValue::new("level", level.name()));
        if let Some((trace, span)) = self.spans.ids(span) {
            kv.push(KeyValue::new("trace_id", trace));
            kv.push(KeyValue::new("span_id", span));
        }
        let severity = match level {
            Level::Debug => Severity::Debug,
            Level::Info => Severity::Info,
            Level::Warn => Severity::Warn,
            Level::Error => Severity::Error,
        };
        let _context = self.runtime.as_ref().map(tokio::runtime::Runtime::enter);
        sink.log(severity, &message.to_string(), kv);
    }
}
