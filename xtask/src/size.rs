// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The 200-line limit.

use crate::scan::{Findings, files};
use std::path::Path;

/// No hand-written file exceeds this, documentation included.
///
/// The limit is about comprehension. A file that fits in two screens can be
/// held in the head while reading it, and a protocol codec nobody can hold in
/// their head is a codec where the off-by-one lives.
pub const MAX_LINES: usize = 200;

/// File kinds the limit applies to.
///
/// Markdown is in the list deliberately. A reference document nobody finishes
/// is as useless as a function nobody can follow, and the same remedy applies:
/// split it along a seam that already exists.
const SIZED_EXTENSIONS: &[&str] = &[
    "rs", "md", "toml", "proto", "yaml", "yml", "just", "hex", "kat", "fbs", "capnp",
];

/// Extensionless files the limit still applies to.
const SIZED_NAMES: &[&str] = &["justfile"];

/// Files that are generated or locked, and so not anyone's design decision.
const EXEMPT_NAMES: &[&str] = &[
    "Cargo.lock",
    "buffers.lock",
    "buf.lock",
    "MODULE.bazel.lock",
];

/// Checks every hand-written file against [`MAX_LINES`].
///
/// # Errors
///
/// When a file is over the limit, or when no file was examined.
pub fn run(root: &Path) -> Result<String, String> {
    let mut findings = Findings::default();
    for rel in files(root) {
        let name = rel.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if EXEMPT_NAMES.contains(&name) || !is_sized(&rel) {
            continue;
        }
        findings.checked();
        let Ok(text) = std::fs::read_to_string(root.join(&rel)) else {
            continue;
        };
        let lines = count_lines(&text);
        if lines > MAX_LINES {
            findings.violation(format!(
                "{} is {lines} lines, over the {MAX_LINES}-line limit; split it along a \
                 seam that already exists, and never by deleting documentation -- a file \
                 that only fits without its comments was doing two jobs",
                rel.display()
            ));
        }
    }
    findings.verdict("size")
}

/// Whether the limit applies to this path.
fn is_sized(rel: &Path) -> bool {
    let name = rel.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if SIZED_NAMES.contains(&name) {
        return true;
    }
    rel.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| SIZED_EXTENSIONS.contains(&ext))
}

/// Counts lines the way a person reading the file would.
///
/// A trailing newline terminates the last line rather than starting another,
/// so a file ending without one still has its final line counted.
fn count_lines(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let breaks = text.matches('\n').count();
    if text.ends_with('\n') {
        breaks
    } else {
        breaks + 1
    }
}

#[cfg(test)]
mod tests {
    use super::count_lines;

    #[test]
    fn a_file_without_a_trailing_newline_still_counts_its_last_line() {
        assert_eq!(count_lines("a\nb\n"), 2);
        assert_eq!(count_lines("a\nb"), 2);
        assert_eq!(count_lines(""), 0);
    }
}
