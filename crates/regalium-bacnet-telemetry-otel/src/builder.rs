// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Configuring and starting the SDK pipeline.

use crate::Otel;
use crate::bridge::Instruments;
use opentelemetry::metrics::MeterProvider;

/// Options for [`Otel`]. Required arguments are positional on
/// [`Otel::builder`]; everything here is optional, so adding an option never
/// breaks a caller.
#[derive(Clone, Debug)]
pub struct Builder {
    service: String,
    version: String,
    otlp: Option<(String, u16)>,
    labels: Vec<(String, String)>,
    console: bool,
}

impl Builder {
    pub(crate) fn new(service: &str, version: &str) -> Self {
        Self {
            service: service.to_owned(),
            version: version.to_owned(),
            otlp: None,
            labels: Vec::new(),
            console: false,
        }
    }

    /// Exports over OTLP gRPC to `host:port`, overriding `telemetry.toml`
    /// and the environment.
    #[must_use]
    pub fn otlp(mut self, host: &str, port: u16) -> Self {
        self.otlp = Some((host.to_owned(), port));
        self
    }

    /// A resource label on every series this process exports, e.g. a site
    /// name. Resource labels identify the exporter, not a point.
    #[must_use]
    pub fn label(mut self, key: &str, value: &str) -> Self {
        self.labels.push((key.to_owned(), value.to_owned()));
        self
    }

    /// Lets the SDK log to stdout. Off by default: the SDK's console logger
    /// writes to stdout, which corrupts a terminal UI and mixes with a CLI's
    /// machine-readable output.
    #[must_use]
    pub fn console(mut self, on: bool) -> Self {
        self.console = on;
        self
    }

    /// Starts the pipeline. Spawns the exporter runtime's worker thread; the
    /// returned [`Otel`] owns it and stops it on [`Otel::close`] or drop.
    ///
    /// # Errors
    ///
    /// When the runtime cannot start or the SDK rejects its configuration.
    /// An unreachable collector is not an error here: export failures are
    /// the SDK's to report, and recording never fails because of one.
    pub fn build(self) -> Result<Otel, String> {
        if !self.console {
            silence_console();
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("telemetry-export")
            .enable_all()
            .build()
            .map_err(|e| format!("telemetry runtime: {e}"))?;
        let context = runtime.enter();
        let mut sdk =
            telemetry::Telemetry::new().with_service(self.service.clone(), self.version.clone());
        if let Some((host, port)) = &self.otlp {
            sdk = sdk.with_otlp(host.clone(), *port);
        }
        for (key, value) in &self.labels {
            sdk = sdk.with_label(key.clone(), value.clone());
        }
        let telemetry = sdk.build().map_err(|e| format!("telemetry SDK: {e}"))?;
        let meter = telemetry
            .meter_provider()
            .map(|p| p.meter("regalium-bacnet"));
        drop(context);
        Ok(Otel {
            instruments: Instruments::new(meter),
            telemetry: Some(telemetry),
            runtime: Some(runtime),
        })
    }
}

/// Claims `log`'s one global logger before the SDK can, with one that
/// discards. `log` accepts exactly one logger per process and the SDK
/// ignores the error when it loses, so this is the supported way to keep its
/// console output off a terminal the caller owns.
fn silence_console() {
    struct Silent;
    impl log::Log for Silent {
        fn enabled(&self, _: &log::Metadata<'_>) -> bool {
            false
        }
        fn log(&self, _: &log::Record<'_>) {}
        fn flush(&self) {}
    }
    static SILENT: Silent = Silent;
    if log::set_logger(&SILENT).is_ok() {
        log::set_max_level(log::LevelFilter::Off);
    }
}
