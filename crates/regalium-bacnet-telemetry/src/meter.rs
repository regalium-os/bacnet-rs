// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Metrics: declared instruments, recorded with declared attributes.
//!
//! An instrument is a `const` declared once beside the code it measures, with
//! its unit and description, so a dashboard author reads what a series means
//! from its declaration rather than guessing from its name. A recording names
//! the instrument and passes [`Attribute`]s built only from declared
//! [`Key`](crate::Key)s — there is no way to label a series with an ad hoc
//! string.

use crate::Attribute;

/// How a recorded value combines with the ones before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Monotonic total: each recording is added. Negative values are a bug
    /// and an adapter may drop them.
    Counter,
    /// Current value: each recording replaces the last, per attribute set.
    /// What a point's present-value is.
    Gauge,
    /// Distribution: each recording is one sample, e.g. a round-trip time.
    Histogram,
}

/// A metric, declared once as a `const`.
///
/// Names follow OpenTelemetry convention, `bacnet.<thing>.<measure>`, and are
/// translated by the exporter (Prometheus turns the dots into underscores and
/// appends the unit).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instrument {
    /// The series name, e.g. `bacnet.point.value`.
    pub name: &'static str,
    /// UCUM unit, e.g. `ms`, `By`, or `1` for a dimensionless count.
    pub unit: &'static str,
    /// What one recorded value means.
    pub description: &'static str,
    /// How recordings combine.
    pub kind: Kind,
}

impl Instrument {
    /// Declares a counter.
    #[must_use]
    pub const fn counter(
        name: &'static str,
        unit: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            unit,
            description,
            kind: Kind::Counter,
        }
    }

    /// Declares a gauge.
    #[must_use]
    pub const fn gauge(name: &'static str, unit: &'static str, description: &'static str) -> Self {
        Self {
            name,
            unit,
            description,
            kind: Kind::Gauge,
        }
    }

    /// Declares a histogram.
    #[must_use]
    pub const fn histogram(
        name: &'static str,
        unit: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            unit,
            description,
            kind: Kind::Histogram,
        }
    }
}

/// Where recordings go. Declared here, by the consumer of the boundary, and
/// satisfied by an adapter — `regalium-bacnet-telemetry-otel` binds it to the
/// the-protobuf-project telemetry SDK.
///
/// # Concurrency
///
/// `Send + Sync` because a driver records from its socket thread and its
/// polling thread at once. Implementations must accept concurrent calls; a
/// recording is fire-and-forget and must never block on export.
pub trait Meter: Send + Sync {
    /// Records `value` against `instrument`, labelled by `attributes`.
    ///
    /// Infallible on purpose: a metric that cannot be exported is the
    /// telemetry pipeline's problem to report, never a reason for a read from
    /// a lift controller to fail.
    fn record(&self, instrument: &Instrument, value: f64, attributes: &[Attribute<'_>]);
}

/// A meter that records nothing, for callers that configure no telemetry.
/// `Default`, zero-sized, and free: it costs one empty call.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopMeter;

impl Meter for NoopMeter {
    fn record(&self, _: &Instrument, _: f64, _: &[Attribute<'_>]) {}
}

#[cfg(test)]
mod tests {
    use super::{Instrument, Kind, Meter, NoopMeter};
    use crate::{Attribute, keys};

    const REQUESTS: Instrument = Instrument::counter("bacnet.test.requests", "1", "Requests sent");

    #[test]
    fn an_instrument_declares_its_kind_at_compile_time() {
        assert_eq!(REQUESTS.kind, Kind::Counter);
        assert_eq!(Instrument::gauge("g", "1", "").kind, Kind::Gauge);
        assert_eq!(Instrument::histogram("h", "ms", "").kind, Kind::Histogram);
    }

    #[test]
    fn the_noop_meter_accepts_anything() {
        let meter: &dyn Meter = &NoopMeter;
        meter.record(
            &REQUESTS,
            1.0,
            &[Attribute::int(keys::DEVICE_INSTANCE, 599)],
        );
    }
}
