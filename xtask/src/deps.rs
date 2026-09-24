// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What may never enter the dependency graph.

use crate::metadata;
use crate::scan::Findings;
use std::path::Path;

/// The crate permitted to name an OpenTelemetry type.
const OTEL_ADAPTER: &str = "regalium-bacnet-telemetry-otel";

/// `links` values that mark a version, not a native library.
///
/// The `links` key exists to let cargo refuse two crates claiming the same
/// native library. A few crates borrow it for an unrelated purpose -- one
/// copy of me per build -- and link no C at all. Each entry here is a crate
/// that was checked by hand; add one only after reading its build script.
const LINK_MARKERS: &[&str] = &["wasm_bindgen"];

/// Dependency name prefixes nobody but [`OTEL_ADAPTER`] may declare.
///
/// Observability goes through the the-protobuf-project telemetry SDK, which
/// owns the exporter lifecycle and the attribute convention. Two things
/// deciding how a span is made is how instrumentation rots.
const OTEL_PREFIXES: &[&str] = &["opentelemetry", "tracing-opentelemetry"];

/// Checks the bans that keep the driver pure Rust and free of a second
/// telemetry stack.
///
/// # Errors
///
/// When a banned crate appears, or when nothing was examined.
pub fn run(root: &Path) -> Result<String, String> {
    let members = metadata::workspace_members(root)?;
    let mut findings = Findings::default();

    for member in &members {
        findings.checked();
        for dep in &member.direct_deps {
            if member.name != OTEL_ADAPTER && is_otel(&dep.name) {
                findings.violation(format!(
                    "{} depends on {}; only {OTEL_ADAPTER} may, and every other layer \
                     records through the telemetry seam instead",
                    member.name, dep.name
                ));
            }
            if dep.name == "cc" && dep.kind == "build" {
                findings.violation(format!(
                    "{} depends on {}, which compiles C. These panels are ARM, and what \
                     keeps `--target armv7-unknown-linux-gnueabihf` a one-flag build is \
                     that nothing in the graph needs a C toolchain",
                    member.name, dep.name
                ));
            }
        }
    }

    // A native library usually arrives transitively, which is the failure no
    // amount of reading our own manifests would reveal. The signal is cargo's
    // `links` key rather than a name ending in -sys: js-sys ends in -sys and
    // binds JavaScript, libc does not and binds a C library.
    for krate in metadata::resolved_crates(root)? {
        findings.checked();
        if let Some(library) = krate.links.as_deref().filter(|l| !LINK_MARKERS.contains(l)) {
            findings.violation(format!(
                "{} is in the resolved graph and links the native library {library}; find \
                 what pulled it in with `cargo tree -i {}` and replace that dependency",
                krate.name, krate.name
            ));
        }
        if krate.name == "cc" {
            findings.violation(
                "cc is in the resolved graph, so some dependency compiles C at build time; \
                 find it with `cargo tree -i cc`. These panels are ARM, and a C toolchain \
                 is what turns a one-flag cross build into a cross-compilation project",
            );
        }
    }

    findings.verdict("deps")
}

/// Whether a dependency name is an OpenTelemetry crate.
fn is_otel(name: &str) -> bool {
    OTEL_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::is_otel;

    #[test]
    fn otel_crates_are_recognised_by_prefix() {
        assert!(is_otel("opentelemetry"));
        assert!(is_otel("opentelemetry_sdk"));
        assert!(is_otel("opentelemetry-otlp"));
        assert!(!is_otel("telemetry"));
    }
}
