// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

// Shared by several test binaries, each of which uses a subset. Rust compiles
// this module separately into each one, so anything the current binary does
// not call reads as dead -- which says nothing about whether it is used.
#![allow(dead_code)]
#![allow(clippy::expect_used, clippy::panic)]

//! Building trees that are deliberately wrong.
//!
//! Damaging a good tree is the only way to know a gate reads what it claims
//! to. A gate exercised solely against a correct repository reports success
//! both when it works and when it does nothing, and those are not the same.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A throwaway tree, removed when the test ends.
pub(crate) struct Tree {
    root: PathBuf,
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

impl Tree {
    /// Creates an empty tree under the system temporary directory.
    pub(crate) fn new(label: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir()
            .join("bacnet-rs-gates")
            .join(format!("{label}-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&root).expect("temporary directory");
        Self { root }
    }

    /// Writes a file, creating the directories above it.
    pub(crate) fn file(&self, rel: &str, contents: &str) -> &Self {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent directory");
        }
        std::fs::write(path, contents).expect("write fixture file");
        self
    }

    /// The tree's root.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The notice every licensed Rust file opens with.
pub(crate) const NOTICE: &str = "// Copyright 2026 RegaliumOS™.\n\
                          // SPDX-License-Identifier: Apache-2.0\n\n";

/// The workspace crates and the dependencies the layer table permits them.
const LEGAL: &[(&str, &[&str])] = &[
    ("regalium-bacnet-telemetry", &[]),
    ("regalium-bacnet-core", &["regalium-bacnet-telemetry"]),
    ("regalium-bacnet-ip", &["regalium-bacnet-core"]),
    ("regalium-bacnet-mstp", &["regalium-bacnet-core"]),
    ("regalium-bacnet-sc", &["regalium-bacnet-core"]),
    ("regalium-bacnet-ipv6", &["regalium-bacnet-core"]),
    ("regalium-bacnet-schema", &["regalium-bacnet-telemetry"]),
    (
        "regalium-bacnet-telemetry-otel",
        &["regalium-bacnet-telemetry"],
    ),
    (
        "regalium-bacnet-runtime",
        &["regalium-bacnet-core", "regalium-bacnet-ip"],
    ),
    ("regalium-bacnet", &["regalium-bacnet-runtime"]),
];

/// A complete, legal workspace that the graph gates pass.
///
/// Tests then damage exactly one manifest, so a failure names the damage
/// rather than some unrelated gap in the fixture.
pub(crate) fn legal_workspace(label: &str) -> Tree {
    let tree = Tree::new(label);
    let members = LEGAL
        .iter()
        .map(|(name, _)| format!("    \"crates/{name}\","))
        .collect::<Vec<_>>()
        .join("\n");
    tree.file(
        "Cargo.toml",
        &format!("[workspace]\nresolver = \"3\"\nmembers = [\n{members}\n]\n"),
    );
    for (name, deps) in LEGAL {
        write_crate(&tree, name, deps);
    }
    tree
}

/// Replaces one crate's manifest, to plant a violation in it.
pub(crate) fn rewrite_crate(tree: &Tree, name: &str, deps: &[&str]) {
    write_crate(tree, name, deps);
}

/// Adds a crate to the fixture workspace under its own name.
///
/// Tests depend on these by path rather than by version so the fixture
/// resolves offline and its graph holds nothing but what the test put there.
pub(crate) fn stub_crate(tree: &Tree, name: &str) {
    let manifest = std::fs::read_to_string(tree.root().join("Cargo.toml")).expect("manifest");
    let members = manifest.replace("\n]", &format!("\n    \"crates/{name}\",\n]"));
    tree.file("Cargo.toml", &members);
    write_crate(tree, name, &[]);
}

fn write_crate(tree: &Tree, name: &str, deps: &[&str]) {
    let dependencies = deps
        .iter()
        .map(|dep| format!("{dep} = {{ path = \"../{dep}\" }}"))
        .collect::<Vec<_>>()
        .join("\n");
    tree.file(
        &format!("crates/{name}/Cargo.toml"),
        &format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\
             edition = \"2024\"\n\n[dependencies]\n{dependencies}\n"
        ),
    );
    tree.file(&format!("crates/{name}/src/lib.rs"), NOTICE);
}
