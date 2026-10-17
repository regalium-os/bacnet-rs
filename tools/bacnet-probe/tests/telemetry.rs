// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! Spans and log lines from real API calls against the simulated gateway:
//! a call nests inside the span it was made in, the `BACnet` requests nest
//! inside the call, a refusal says which and why, and a warning lands in
//! the span it happened in.

mod common;

use bacnet_probe::api::name;
use bacnet_probe::sim;
use regalium_bacnet_schema::object as obj;
use regalium_bacnet_telemetry::{Attribute, AttributeValue, Level, Logger, SpanId, Tracer};
use std::fmt::Display;
use std::sync::{Arc, Mutex};

/// One span as recorded: id, name, parent, attributes, error.
#[derive(Clone, Debug, Default)]
struct Rec {
    id: u64,
    name: &'static str,
    parent: u64,
    attrs: Vec<(String, String)>,
    error: Option<String>,
}

#[derive(Default)]
struct Recorder {
    spans: Mutex<Vec<Rec>>,
    lines: Mutex<Vec<(Level, String, u64)>>,
}

impl Tracer for Recorder {
    fn start(&self, name: &'static str, parent: SpanId) -> SpanId {
        let mut spans = self.spans.lock().unwrap();
        let id = u64::try_from(spans.len()).unwrap() + 1;
        spans.push(Rec {
            id,
            name,
            parent: parent.0,
            ..Rec::default()
        });
        SpanId(id)
    }
    fn attribute(&self, span: SpanId, a: Attribute<'_>) {
        let v = match a.value {
            AttributeValue::Str(s) => s.to_owned(),
            AttributeValue::Int(n) => n.to_string(),
            AttributeValue::Bool(b) => b.to_string(),
        };
        self.spans.lock().unwrap()[usize::try_from(span.0).unwrap() - 1]
            .attrs
            .push((a.key.name().to_owned(), v));
    }
    fn end(&self, span: SpanId, error: Option<&dyn Display>) {
        self.spans.lock().unwrap()[usize::try_from(span.0).unwrap() - 1].error =
            error.map(ToString::to_string);
    }
}

impl Logger for Recorder {
    fn enabled(&self, _: Level) -> bool {
        true
    }
    fn log(&self, level: Level, message: &dyn Display, span: SpanId, _: &[Attribute<'_>]) {
        self.lines
            .lock()
            .unwrap()
            .push((level, message.to_string(), span.0));
    }
}

impl Recorder {
    fn named(&self, name: &str) -> Vec<Rec> {
        self.spans
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.name == name)
            .cloned()
            .collect()
    }
    fn attr(rec: &Rec, key: &str) -> Option<String> {
        rec.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    }
}

#[test]
fn a_call_nests_in_its_cycle_and_its_requests_nest_in_the_call() {
    let rec = Arc::new(Recorder::default());
    let mut api = common::api_observed(rec.clone(), rec.clone());
    let cycle = api.client().enter("bacnet.e2e.cycle");
    api.get_object(&obj::GetObjectRequest {
        name: name::object(599, sim::FAN),
    })
    .unwrap();
    api.client().exit(cycle, None);

    let call = &rec.named("bacnet.api.GetObject")[0];
    assert_eq!(call.parent, cycle.0, "the call is inside the cycle");
    let discover = &rec.named("bacnet.client.discover")[0];
    assert_eq!(
        discover.parent, call.id,
        "finding the device is part of the call"
    );
    let read = &rec.named("bacnet.client.readProperty")[0];
    assert_eq!(read.parent, call.id);
    assert_eq!(
        Recorder::attr(read, "bacnet.object.type").as_deref(),
        Some("analog-output")
    );
    assert_eq!(
        Recorder::attr(read, "bacnet.property.identifier").as_deref(),
        Some("object-name")
    );
    assert_eq!(
        Recorder::attr(read, "bacnet.request.attempts").as_deref(),
        Some("1")
    );
    assert_eq!(
        Recorder::attr(call, "bacnet.api.status").as_deref(),
        Some("ok")
    );
    let lines = rec.lines.lock().unwrap();
    assert!(lines.iter().any(|(l, m, s)| *l == Level::Info
        && m.contains("1 device(s) answered")
        && *s == discover.id));
}

#[test]
fn a_refusal_names_the_error_on_its_request_and_warns_in_its_call() {
    let rec = Arc::new(Recorder::default());
    let mut api = common::api_observed(rec.clone(), rec.clone());
    let input = obj::Property {
        name: name::property(599, sim::CAR_LOAD, 85),
        value: Some(bacnet_probe::api::to_schema(&bacnet_probe::Value::Real(
            1.0,
        ))),
        ..Default::default()
    };
    let refused = api.update_property(&obj::UpdatePropertyRequest {
        property: Some(input),
        update_mask: None,
    });
    assert!(refused.is_err());

    let write = &rec.named("bacnet.client.writeProperty")[0];
    assert_eq!(
        Recorder::attr(write, "bacnet.error.code").as_deref(),
        Some("write-access-denied")
    );
    assert_eq!(
        Recorder::attr(write, "bacnet.outcome").as_deref(),
        Some("error")
    );
    assert!(
        write
            .error
            .as_deref()
            .unwrap()
            .contains("write-access-denied")
    );
    let call = &rec.named("bacnet.api.UpdateProperty")[0];
    assert_eq!(
        Recorder::attr(call, "bacnet.api.status").as_deref(),
        Some("FailedPrecondition")
    );
    let lines = rec.lines.lock().unwrap();
    let warn = lines.iter().find(|(l, ..)| *l == Level::Warn).unwrap();
    assert!(
        warn.1.starts_with("UpdateProperty:") && warn.2 == call.id,
        "{warn:?}"
    );
}
