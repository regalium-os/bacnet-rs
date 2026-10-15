// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Binds the telemetry seam to the the-protobuf-project telemetry SDK.
//!
//! This is the only crate in the workspace allowed to name an OpenTelemetry
//! type, and the reason no other crate has to. `cargo xtask deps` fails the
//! build if `opentelemetry` appears anywhere else.
//!
//! The SDK's `Tracer` and `Span` are concrete structs rather than traits, so
//! the binding is a hand-written implementation of the seam's traits over
//! them — there is no structural adapter to be had, unlike `osdp-go`, whose
//! Go SDK exposes a method value that can be matched by shape.
//!
//! # Metrics
//!
//! [`Otel`] implements the seam's [`Meter`]. Each declared
//! [`Instrument`](regalium_bacnet_telemetry::Instrument) becomes one SDK
//! instrument, created on first use and cached, and each seam attribute
//! becomes an OpenTelemetry `KeyValue` under its declared key. The SDK's own
//! metrics API takes a bare name and value, so the binding goes through the
//! meter provider the SDK exposes instead — that is what carries the labels.
//!
//! Export is OTLP to whatever the SDK is configured for: [`Builder::otlp`],
//! else `telemetry.toml` / `TELEMETRY_*` environment variables, as the SDK
//! documents. The export interval is the OpenTelemetry default unless
//! `OTEL_METRIC_EXPORT_INTERVAL` (milliseconds) says otherwise.
//!
//! # Layer contract
//!
//! Owns: construction and shutdown of the SDK pipeline, the Tokio runtime its
//! gRPC exporter needs, translation of seam attributes into SDK attributes,
//! and the mapping from seam instruments onto the SDK's.
//!
//! Must not: be imported by the core, a datalink, or the schema crate. An
//! application chooses a telemetry backend; a protocol codec does not. The
//! SDK's MCAP recorder links zstd and lz4, which `cargo xtask deps` accepts
//! beneath this crate and the application built on it, and nowhere else.
//!
//! Allowed imports: `regalium-bacnet-telemetry`, and the SDK.

mod bridge;
mod builder;

use bridge::Instruments;
pub use builder::Builder;
use regalium_bacnet_telemetry::{Attribute, Instrument, Meter};

/// A running SDK pipeline that the seam's [`Meter`] records into.
///
/// Built with [`Otel::builder`]. Dropping it flushes and shuts the pipeline
/// down, as [`Otel::close`] does.
///
/// # Concurrency
///
/// `Send + Sync`, and correct to share: recording takes a short lock on the
/// instrument cache only the first time an instrument is seen, then records
/// through the SDK, which is itself thread-safe. Recording never blocks on
/// export; the SDK exports from its own thread.
pub struct Otel {
    instruments: Instruments,
    telemetry: Option<telemetry::Telemetry>,
    runtime: Option<tokio::runtime::Runtime>,
}

impl Otel {
    /// Starts configuring a pipeline for `service` at `version`.
    #[must_use]
    pub fn builder(service: &str, version: &str) -> Builder {
        Builder::new(service, version)
    }

    /// Records into an OpenTelemetry meter the caller owns, with no SDK
    /// pipeline of ours — for tests, or an application that already runs its
    /// own OpenTelemetry provider.
    #[must_use]
    pub fn from_meter(meter: opentelemetry::metrics::Meter) -> Self {
        Self {
            instruments: Instruments::new(Some(meter)),
            telemetry: None,
            runtime: None,
        }
    }

    /// Whether recordings go anywhere. False when the SDK was configured with
    /// metrics disabled; recording is then a no-op.
    #[must_use]
    pub fn is_exporting(&self) -> bool {
        self.instruments.is_live()
    }

    /// Pushes everything recorded so far to the exporter now.
    ///
    /// # Errors
    ///
    /// When the SDK reports the flush failed, e.g. the collector is down.
    pub fn flush(&self) -> Result<(), String> {
        let Some(telemetry) = &self.telemetry else {
            return Ok(());
        };
        let _context = self.runtime.as_ref().map(tokio::runtime::Runtime::enter);
        telemetry.flush().map_err(|e| e.to_string())
    }

    /// Flushes and shuts the pipeline down. Idempotent, and safe after a
    /// failed flush: the second call finds nothing left to close.
    pub fn close(&mut self) {
        if let Some(telemetry) = self.telemetry.take() {
            // The exporter's shutdown talks to its connection tasks, which
            // live on the runtime; drop it while the runtime still runs.
            let _context = self.runtime.as_ref().map(tokio::runtime::Runtime::enter);
            drop(telemetry);
        }
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(std::time::Duration::from_secs(2));
        }
    }
}

impl Meter for Otel {
    fn record(&self, instrument: &Instrument, value: f64, attributes: &[Attribute<'_>]) {
        self.instruments.record(instrument, value, attributes);
    }
}

impl Drop for Otel {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests;
