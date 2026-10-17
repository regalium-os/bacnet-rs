// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used)]

//! The transaction span, through a tracer that remembers what it was told:
//! one span from request to end, under the caller's, with the declared
//! attributes and never the request or the answer.

use regalium_bacnet_core::tsm::{Config, Request, Step, Tsm};
use regalium_bacnet_telemetry::{Attribute, AttributeValue, SpanId, Tracer};
use std::fmt::Display;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn drain(tsm: &mut Tsm<u8>, now: Duration) {
    while !matches!(tsm.poll(now), Step::Idle { .. }) {}
}

#[derive(Default)]
struct Recorder(Mutex<Vec<String>>);

impl Tracer for Recorder {
    fn start(&self, name: &'static str, parent: SpanId) -> SpanId {
        self.0
            .lock()
            .unwrap()
            .push(format!("start {name} under {}", parent.0));
        SpanId(9)
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
fn a_transaction_is_one_span_from_request_to_its_end() {
    let r = Arc::new(Recorder::default());
    let mut tsm = Tsm::new(Config::default().retries(1)).with_tracer(r.clone());
    tsm.request(
        1,
        Request::new(12, vec![0x0c]).within(SpanId(5)),
        Duration::ZERO,
    )
    .unwrap();
    drain(&mut tsm, Duration::from_secs(3));
    drain(&mut tsm, Duration::from_secs(6));
    assert_eq!(
        *r.0.lock().unwrap(),
        [
            "start bacnet.tsm.transaction under 5",
            "bacnet.invoke_id=0",
            "bacnet.apdu.service_choice=12",
            "bacnet.apdu.segmented=false",
            "bacnet.request.attempts=2",
            "bacnet.outcome=timeout",
            "fail no answer after 2 attempts",
        ]
    );
}
