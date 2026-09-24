// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Finding the tree, walking it, and refusing to pass when it is empty.

use std::path::{Path, PathBuf};

/// Directories no gate ever descends into.
///
/// `generated` is here because generated output is rewritten wholesale: it
/// carries no licence notice of ours and its line count is not a design
/// decision. Everything else is machinery rather than source.
const SKIP_DIRS: &[&str] = &["target", ".git", "generated", "node_modules"];

/// Locates the workspace root by walking up from this crate.
///
/// Fails rather than falling back to the current directory. A runner that
/// hides the tree — a sandbox that did not declare the sources as inputs, for
/// one — must produce a failure and not a green tick, because a gate reporting
/// success over nothing is worse than no gate: it is believed.
///
/// # Errors
///
/// When no ancestor directory holds a `Cargo.toml` declaring `[workspace]`.
pub fn repo_root() -> Result<PathBuf, String> {
    let start = Path::new(env!("CARGO_MANIFEST_DIR"));
    for dir in start.ancestors() {
        let manifest = dir.join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        if text.contains("[workspace]") {
            return Ok(dir.to_path_buf());
        }
    }
    Err(format!(
        "no workspace manifest above {}; the gates cannot see the tree they \
         are meant to measure, which is a failure and not a pass",
        start.display()
    ))
}

/// Every file under `root`, skipping [`SKIP_DIRS`] and Bazel's symlinks.
///
/// Returns paths relative to `root`, sorted, so a failure report reads the
/// same on every machine.
#[must_use]
pub fn files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect(root, root, &mut found);
    found.sort();
    found
}

fn collect(root: &Path, dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if path.is_dir() {
            // Whole path elements, never bare prefixes: a prefix test for
            // ".git" would swallow ".github" and quietly stop checking CI.
            if SKIP_DIRS.contains(&name) || name.starts_with("bazel-") {
                continue;
            }
            collect(root, &path, found);
        } else if let Ok(rel) = path.strip_prefix(root) {
            found.push(rel.to_path_buf());
        }
    }
}

/// What a gate saw and what it objected to.
///
/// The count matters as much as the objections: a gate that examined nothing
/// has not passed, it has failed to run.
#[derive(Debug, Default)]
pub struct Findings {
    checked: usize,
    violations: Vec<String>,
}

impl Findings {
    /// Records that one more item was examined.
    pub const fn checked(&mut self) {
        self.checked += 1;
    }

    /// Records an objection.
    pub fn violation(&mut self, message: impl Into<String>) {
        self.violations.push(message.into());
    }

    /// How many items this gate examined.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.checked
    }

    /// Turns findings into a verdict, failing an empty scan.
    ///
    /// # Errors
    ///
    /// When anything was found, or when nothing was examined at all.
    pub fn verdict(self, gate: &str) -> Result<String, String> {
        if self.checked == 0 {
            return Err(format!(
                "{gate}: examined nothing, so this gate is checking nothing"
            ));
        }
        if self.violations.is_empty() {
            return Ok(format!("{gate}: {} items, clean", self.checked));
        }
        Err(format!(
            "{gate}: {} of {} items failed\n  {}",
            self.violations.len(),
            self.checked,
            self.violations.join("\n  ")
        ))
    }
}
