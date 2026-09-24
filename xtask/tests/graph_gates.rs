// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

// A test asserts by panicking, so the workspace's never-panic lints are not
// the rule here: an expect that cannot fire is how a test states what it
// requires, and one that fires is the test doing its job.
#![allow(clippy::expect_used, clippy::panic)]

//! The arch, purity and deps gates, exercised against workspaces that are wrong.
//!
//! Each test starts from a legal workspace and damages exactly one manifest,
//! so a failure names the damage rather than some unrelated gap in the fixture.

mod common;

use common::{legal_workspace, rewrite_crate};
use xtask::{arch, deps, purity};

#[test]
fn a_legal_workspace_passes_all_three() {
    let tree = legal_workspace("legal");

    arch::run(tree.root()).expect("the fixture models the layer table");
    purity::run(tree.root()).expect("the fixture's pure crates depend only on the seam");
    deps::run(tree.root()).expect("the fixture names no banned crate");
}

#[test]
fn arch_rejects_a_dependency_that_inverts_the_hexagon() {
    let tree = legal_workspace("arch-inverted");
    rewrite_crate(&tree, "regalium-bacnet-core", &["regalium-bacnet-ip"]);

    let error = arch::run(tree.root()).expect_err("the core may not depend on a datalink");
    assert!(error.contains("inverts the hexagon"), "{error}");
}

#[test]
fn arch_rejects_a_crate_missing_from_the_layer_table() {
    let tree = legal_workspace("arch-ungoverned");
    tree.file(
        "Cargo.toml",
        "[workspace]\nresolver = \"3\"\nmembers = [\"crates/regalium-bacnet-stray\"]\n",
    );
    rewrite_crate(&tree, "regalium-bacnet-stray", &[]);

    let error = arch::run(tree.root()).expect_err("an ungoverned crate must be reported");
    assert!(error.contains("not in the layer table"), "{error}");
}

#[test]
fn arch_reports_a_layer_that_has_gone_missing() {
    let tree = legal_workspace("arch-absent");
    tree.file(
        "Cargo.toml",
        "[workspace]\nresolver = \"3\"\nmembers = [\"crates/regalium-bacnet-core\"]\n",
    );

    let error =
        arch::run(tree.root()).expect_err("a table describing an absent tree checks nothing");
    assert!(error.contains("absent from the workspace"), "{error}");
}

#[test]
fn purity_rejects_a_dependency_on_the_pure_core() {
    let tree = legal_workspace("purity-dep");
    rewrite_crate(&tree, "regalium-bacnet-core", &["regalium-bacnet-schema"]);

    let error = purity::run(tree.root()).expect_err("a pure layer's only dependency is the seam");
    assert!(
        error.contains("pure layer's only permitted dependency"),
        "{error}"
    );
}

#[test]
fn purity_rejects_io_in_a_pure_layer() {
    let tree = legal_workspace("purity-io");
    tree.file(
        "crates/regalium-bacnet-core/src/lib.rs",
        &format!("{}use std::fs::File;\n", common::NOTICE),
    );

    let error = purity::run(tree.root()).expect_err("the pure core reads no files");
    assert!(error.contains("std::fs"), "{error}");
}

#[test]
fn purity_rejects_a_clock_read_in_a_pure_layer() {
    let tree = legal_workspace("purity-clock");
    tree.file(
        "crates/regalium-bacnet-core/src/lib.rs",
        &format!(
            "{}fn now() {{ std::time::Instant::now(); }}\n",
            common::NOTICE
        ),
    );

    let error = purity::run(tree.root()).expect_err("time is injected, never read");
    assert!(error.contains("time is injected"), "{error}");
}

#[test]
fn deps_rejects_opentelemetry_outside_the_adapter() {
    let tree = legal_workspace("deps-otel");
    common::stub_crate(&tree, "opentelemetry");
    rewrite_crate(&tree, "regalium-bacnet-core", &["opentelemetry"]);

    let error = deps::run(tree.root()).expect_err("only the adapter may name OpenTelemetry");
    assert!(
        error.contains("records through the telemetry seam"),
        "{error}"
    );
}

#[test]
fn deps_permits_opentelemetry_inside_the_adapter() {
    let tree = legal_workspace("deps-otel-ok");
    common::stub_crate(&tree, "opentelemetry");
    rewrite_crate(&tree, "regalium-bacnet-telemetry-otel", &["opentelemetry"]);

    deps::run(tree.root()).expect("the adapter exists precisely to hold this dependency");
}

#[test]
fn deps_rejects_a_crate_that_compiles_c() {
    let tree = legal_workspace("deps-cc");
    common::stub_crate(&tree, "cc");
    tree.file(
        "crates/regalium-bacnet-ip/Cargo.toml",
        "[package]\nname = \"regalium-bacnet-ip\"\nversion = \"0.1.0\"\n\
         edition = \"2024\"\n\n[build-dependencies]\ncc = { path = \"../cc\" }\n",
    );

    let error = deps::run(tree.root()).expect_err("a C toolchain breaks the one-flag cross build");
    assert!(error.contains("compiles C"), "{error}");
}

#[test]
fn deps_rejects_a_transitive_native_library() {
    let tree = legal_workspace("deps-links");
    common::stub_crate(&tree, "openssl-sys");
    tree.file(
        "crates/openssl-sys/Cargo.toml",
        "[package]\nname = \"openssl-sys\"\nversion = \"0.1.0\"\n\
         edition = \"2024\"\nlinks = \"ssl\"\nbuild = \"build.rs\"\n",
    );
    tree.file(
        "crates/openssl-sys/build.rs",
        &format!("{}fn main() {{}}\n", common::NOTICE),
    );
    rewrite_crate(&tree, "regalium-bacnet-ip", &["openssl-sys"]);

    let error = deps::run(tree.root()).expect_err("a native library breaks the cross build");
    assert!(error.contains("links the native library ssl"), "{error}");
}

#[test]
fn deps_permits_a_links_key_used_as_a_version_marker() {
    let tree = legal_workspace("deps-marker");
    common::stub_crate(&tree, "wasm-bindgen-shared");
    tree.file(
        "crates/wasm-bindgen-shared/Cargo.toml",
        "[package]\nname = \"wasm-bindgen-shared\"\nversion = \"0.1.0\"\n\
         edition = \"2024\"\nlinks = \"wasm_bindgen\"\nbuild = \"build.rs\"\n",
    );
    tree.file(
        "crates/wasm-bindgen-shared/build.rs",
        &format!("{}fn main() {{}}\n", common::NOTICE),
    );
    rewrite_crate(&tree, "regalium-bacnet-ip", &["wasm-bindgen-shared"]);

    deps::run(tree.root()).expect("wasm_bindgen names a version marker, not a C library");
}
