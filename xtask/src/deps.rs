// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What may never enter the dependency graph, and where the one exception is.

use crate::graph::Graph;
use crate::metadata;
use crate::scan::Findings;
use std::collections::BTreeSet;
use std::path::Path;

/// The crate permitted to name an OpenTelemetry type.
const OTEL_ADAPTER: &str = "regalium-bacnet-telemetry-otel";

/// Crates whose graph may hold an [`AUDITED_NATIVE`] library: the telemetry
/// adapter, and the application built on it.
///
/// Everything else -- the core, every datalink, the runtime, the schema, the
/// seam and the facade -- stays pure Rust, so a building integrator who
/// depends on the driver keeps a one-flag cross build. Only a caller that
/// opts into exporting telemetry through the SDK takes on a C toolchain.
pub const NATIVE_PERMITTED: &[&str] = &[OTEL_ADAPTER, "regalium-bacnet-probe"];

/// Native libraries read by hand and accepted, beneath [`NATIVE_PERMITTED`]
/// only: `(crate, links, why)`.
///
/// Both arrive through the telemetry SDK, which records MCAP for Foxglove
/// and compresses it. They compile vendored C with `cc`; nothing here links
/// a system library. Add an entry only after reading the crate's build script.
pub const AUDITED_NATIVE: &[(&str, &str, &str)] = &[
    (
        "zstd-sys",
        "zstd",
        "zstd for MCAP chunks, via telemetry -> mcap -> zstd",
    ),
    (
        "lz4-sys",
        "lz4",
        "lz4 for MCAP chunks, via telemetry -> mcap -> lz4",
    ),
];

/// Every target this repository builds: CI's cross matrix, and the release
/// platforms of the probe. The graph is checked as resolved for each.
pub const TARGETS: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "armv7-unknown-linux-gnueabihf",
    "x86_64-pc-windows-gnu",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
];

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
    // The same defect usually shows on every target; report it once.
    let mut native = BTreeSet::new();
    for target in TARGETS {
        let graph = Graph::resolve(root, target)?;
        check_native(&graph, &mut findings, &mut native);
    }
    for message in native {
        findings.violation(message);
    }

    findings.verdict("deps")
}

/// Native code in each member's closure, against [`NATIVE_PERMITTED`] and
/// [`AUDITED_NATIVE`].
fn check_native(graph: &Graph, findings: &mut Findings, out: &mut BTreeSet<String>) {
    for member in graph.member_names() {
        let permitted = NATIVE_PERMITTED.contains(&member.as_str());
        for node in graph.closure(&member) {
            findings.checked();
            let audited = AUDITED_NATIVE.iter().any(|(name, _, _)| *name == node.name);
            if let Some(library) = node.links.as_deref().filter(|l| !LINK_MARKERS.contains(l)) {
                let accepted = permitted
                    && AUDITED_NATIVE
                        .iter()
                        .any(|(n, l, _)| *n == node.name && *l == library);
                if !accepted {
                    out.insert(native(&member, &node.name, library, permitted));
                }
            }
            if node.name == "cc" && !permitted {
                out.insert(format!(
                    "{member} pulls in cc, so some dependency compiles C at build time; find \
                     it with `cargo tree -p {member} -i cc`. These panels are ARM, and a C \
                     toolchain is what turns a one-flag cross build into a cross-compilation \
                     project"
                ));
            }
            if permitted && !audited && node.build_deps.iter().any(|d| d == "cc") {
                out.insert(format!(
                    "{} compiles C with cc beneath {member} and is not an audited native \
                     library; read its build script, then add it to AUDITED_NATIVE with the \
                     reason, or replace the dependency",
                    node.name
                ));
            }
        }
    }
}

fn native(member: &str, krate: &str, library: &str, permitted: bool) -> String {
    let rule = if permitted {
        "it is not in AUDITED_NATIVE; read its build script before adding it there".to_owned()
    } else {
        format!(
            "native code is accepted only beneath {NATIVE_PERMITTED:?}, so the driver itself \
             stays a one-flag cross build"
        )
    };
    format!(
        "{member} pulls in {krate}, which links the native library {library}; {rule}. Find \
         the path with `cargo tree -p {member} -i {krate}`"
    )
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
