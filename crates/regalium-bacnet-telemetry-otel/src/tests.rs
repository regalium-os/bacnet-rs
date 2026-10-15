// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

// A test asserts by panicking; the never-panic lints are not the rule here.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! Seam recordings, through the bridge, into an in-memory SDK exporter: the
//! labels must arrive under their declared keys, with their declared types.

use crate::Otel;
use opentelemetry::metrics::MeterProvider;
use opentelemetry::{Key, Value};
use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};
use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};
use regalium_bacnet_telemetry::{Attribute, Instrument, Meter, keys};

const VALUE: Instrument = Instrument::gauge("bacnet.point.value", "1", "Present-value");
const REQUESTS: Instrument = Instrument::counter("bacnet.requests", "1", "Requests");

fn pipeline() -> (Otel, SdkMeterProvider, InMemoryMetricExporter) {
    let exporter = InMemoryMetricExporter::default();
    let provider = SdkMeterProvider::builder()
        .with_reader(PeriodicReader::builder(exporter.clone()).build())
        .build();
    (Otel::from_meter(provider.meter("test")), provider, exporter)
}

/// One exported data point: metric name, its labels, its value.
type Point = (String, Vec<(Key, Value)>, f64);

/// Every data point exported, gauges and sums alike.
fn exported(provider: &SdkMeterProvider, exporter: &InMemoryMetricExporter) -> Vec<Point> {
    provider.force_flush().unwrap();
    let mut out = Vec::new();
    for resource in exporter.get_finished_metrics().unwrap() {
        for metric in resource
            .scope_metrics()
            .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        {
            let labels =
                |kv: &mut dyn Iterator<Item = &opentelemetry::KeyValue>| -> Vec<(Key, Value)> {
                    kv.map(|kv| (kv.key.clone(), kv.value.clone())).collect()
                };
            match metric.data() {
                AggregatedMetrics::F64(MetricData::Gauge(g)) => {
                    for p in g.data_points() {
                        out.push((
                            metric.name().to_owned(),
                            labels(&mut p.attributes()),
                            p.value(),
                        ));
                    }
                }
                AggregatedMetrics::F64(MetricData::Sum(s)) => {
                    for p in s.data_points() {
                        out.push((
                            metric.name().to_owned(),
                            labels(&mut p.attributes()),
                            p.value(),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

#[test]
fn a_gauge_arrives_with_every_label_under_its_declared_key_and_type() {
    let (otel, provider, exporter) = pipeline();
    otel.record(
        &VALUE,
        21.5,
        &[
            Attribute::int(keys::DEVICE_INSTANCE, 599),
            Attribute::str(keys::OBJECT_TYPE, "analog-input"),
            Attribute::bool(keys::STATUS_FLAG, true),
        ],
    );
    let all = exported(&provider, &exporter);
    let (_, labels, value) = all
        .iter()
        .find(|(n, _, _)| n == "bacnet.point.value")
        .expect("the gauge is exported");
    assert!((value - 21.5).abs() < f64::EPSILON);
    assert!(
        labels.contains(&(Key::from("bacnet.device.instance"), Value::I64(599))),
        "{labels:?}"
    );
    assert!(
        labels.contains(&(Key::from("bacnet.object.type"), Value::from("analog-input"))),
        "{labels:?}"
    );
    assert!(
        labels.contains(&(Key::from("bacnet.status.flag"), Value::Bool(true))),
        "{labels:?}"
    );
}

#[test]
fn a_counter_accumulates_per_label_set_and_ignores_a_negative_add() {
    let (otel, provider, exporter) = pipeline();
    let ok = [Attribute::str(keys::OUTCOME, "ok")];
    otel.record(&REQUESTS, 1.0, &ok);
    otel.record(&REQUESTS, 2.0, &ok);
    otel.record(&REQUESTS, -5.0, &ok);
    otel.record(&REQUESTS, 1.0, &[Attribute::str(keys::OUTCOME, "timeout")]);
    let all = exported(&provider, &exporter);
    let total = |outcome: &str| {
        all.iter()
            .find(|(n, l, _)| {
                n == "bacnet.requests"
                    && l.contains(&(Key::from("bacnet.outcome"), Value::from(outcome.to_owned())))
            })
            .map(|(_, _, v)| *v)
    };
    assert_eq!(total("ok"), Some(3.0));
    assert_eq!(total("timeout"), Some(1.0));
}

#[test]
fn closing_twice_is_harmless() {
    let (mut otel, _provider, _exporter) = pipeline();
    assert!(otel.is_exporting());
    otel.close();
    otel.close();
    assert!(otel.flush().is_ok());
}
