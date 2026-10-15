// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Seam instruments and attributes, translated into the SDK's.

use opentelemetry::KeyValue;
use opentelemetry::metrics::{Counter, Gauge, Histogram, Meter};
use regalium_bacnet_telemetry::{Attribute, AttributeValue, Instrument, Kind};
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

/// One SDK instrument per declared seam instrument.
enum Sdk {
    Counter(Counter<f64>),
    Gauge(Gauge<f64>),
    Histogram(Histogram<f64>),
}

/// Lazily created SDK instruments, keyed by declared name.
///
/// Created on first use rather than up front, so the seam need not know the
/// set of instruments in advance and an application that records nothing
/// registers nothing.
pub(crate) struct Instruments {
    meter: Option<Meter>,
    cache: Mutex<HashMap<&'static str, Sdk>>,
}

impl Instruments {
    pub(crate) fn new(meter: Option<Meter>) -> Self {
        Self {
            meter,
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn is_live(&self) -> bool {
        self.meter.is_some()
    }

    pub(crate) fn record(&self, instrument: &Instrument, value: f64, attributes: &[Attribute<'_>]) {
        let Some(meter) = &self.meter else { return };
        let labels: Vec<KeyValue> = attributes.iter().map(key_value).collect();
        // A poisoned lock means another thread panicked mid-insert; the map
        // is still a valid cache, so recover it rather than lose metrics.
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        let sdk = cache
            .entry(instrument.name)
            .or_insert_with(|| create(meter, instrument));
        match sdk {
            // A counter is monotonic; a negative add is a caller bug that
            // the SDK would silently drop, so drop it here, explicitly.
            Sdk::Counter(c) if value >= 0.0 => c.add(value, &labels),
            Sdk::Counter(_) => {}
            Sdk::Gauge(g) => g.record(value, &labels),
            Sdk::Histogram(h) => h.record(value, &labels),
        }
    }
}

fn create(meter: &Meter, i: &Instrument) -> Sdk {
    match i.kind {
        Kind::Counter => Sdk::Counter(
            meter
                .f64_counter(i.name)
                .with_unit(i.unit)
                .with_description(i.description)
                .build(),
        ),
        Kind::Gauge => Sdk::Gauge(
            meter
                .f64_gauge(i.name)
                .with_unit(i.unit)
                .with_description(i.description)
                .build(),
        ),
        Kind::Histogram => Sdk::Histogram(
            meter
                .f64_histogram(i.name)
                .with_unit(i.unit)
                .with_description(i.description)
                .build(),
        ),
    }
}

fn key_value(a: &Attribute<'_>) -> KeyValue {
    let key = a.key.name();
    match a.value {
        AttributeValue::Str(s) => KeyValue::new(key, s.to_owned()),
        AttributeValue::Int(n) => KeyValue::new(key, n),
        AttributeValue::Bool(b) => KeyValue::new(key, b),
    }
}
