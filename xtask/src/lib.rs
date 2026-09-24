// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The conformance gates, as a library so they can be tested against a tree
//! that is deliberately wrong.
//!
//! Every gate here is paired with a negative test. A gate tested solely
//! against correct input reports success both when it works and when it does
//! nothing, and those are not the same thing — this is the second reason each
//! gate also fails when it examined nothing at all.

pub mod arch;
pub mod deps;
pub mod licence;
pub mod metadata;
pub mod purity;
pub mod scan;
pub mod size;

use std::path::Path;

/// A gate: a name and the check it runs over a tree.
pub type Gate = (&'static str, fn(&Path) -> Result<String, String>);

/// Every gate, in the order `just gates` runs them.
///
/// Cheapest and most local first: a file over the limit or missing its notice
/// is a one-line fix, and reporting it before spending a `cargo metadata` on
/// the graph keeps the loop short.
pub const GATES: &[Gate] = &[
    ("size", size::run),
    ("licence", licence::run),
    ("arch", arch::run),
    ("purity", purity::run),
    ("deps", deps::run),
];
