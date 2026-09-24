// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Every hand-written file carries the Apache-2.0 notice.

use crate::scan::{Findings, files};
use std::path::Path;

/// The identifier every licensed file declares.
pub const SPDX: &str = "SPDX-License-Identifier: Apache-2.0";

/// The copyright line that precedes it.
pub const COPYRIGHT: &str = "Copyright 2026 RegaliumOS";

/// How many lines from the top the notice must appear within.
///
/// An identifier buried halfway down a file is not a licence notice, and a
/// scanner looking for one will not find it there either.
const HEADER_LINES: usize = 4;

/// File extension to the comment syntax its notice is written in.
const BY_EXTENSION: &[(&str, &str)] = &[
    ("rs", "//"),
    ("proto", "//"),
    ("fbs", "//"),
    ("capnp", "#"),
    ("toml", "#"),
    ("yaml", "#"),
    ("yml", "#"),
    ("just", "#"),
    ("hex", "#"),
    ("kat", "#"),
];

/// Extensionless files that carry a notice, and how.
const BY_NAME: &[(&str, &str)] = &[("justfile", "#"), (".gitattributes", "#")];

/// Trees whose contents are generated or locked, so not ours to notice.
const UNLICENSED: &[&str] = &["generated", "target", ".git"];

/// Files that are pure data or machine-owned.
const UNLICENSED_NAMES: &[&str] = &[
    "Cargo.lock",
    "buffers.lock",
    "buf.lock",
    "MODULE.bazel.lock",
    ".gitignore",
];

/// Checks the notice on every file kind that carries one.
///
/// # Errors
///
/// When a notice is missing or malformed, or when nothing was examined.
pub fn run(root: &Path) -> Result<String, String> {
    let mut findings = Findings::default();
    for rel in files(root) {
        let name = rel.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if UNLICENSED_NAMES.contains(&name) || is_unlicensed(&rel) {
            continue;
        }
        let Some(prefix) = comment_prefix(&rel) else {
            continue;
        };
        findings.checked();
        let Ok(text) = std::fs::read_to_string(root.join(&rel)) else {
            continue;
        };
        if let Some(missing) = missing_notice(&text, prefix) {
            findings.violation(format!(
                "{} is missing {missing} in its first {HEADER_LINES} lines; open it with \
                 `{prefix} {COPYRIGHT}™.` then `{prefix} {SPDX}`, blank line after",
                rel.display()
            ));
        }
    }
    findings.verdict("licence")
}

/// Which notice line, if any, this file fails to declare.
fn missing_notice(text: &str, prefix: &str) -> Option<&'static str> {
    let head: Vec<&str> = text.lines().take(HEADER_LINES).collect();
    let declares = |needle: &str| {
        head.iter()
            .any(|line| line.trim_start().starts_with(prefix) && line.contains(needle))
    };
    match (declares(COPYRIGHT), declares(SPDX)) {
        (true, true) => None,
        (false, true) => Some("the copyright line"),
        (true, false) => Some("the SPDX identifier"),
        (false, false) => Some("the licence notice"),
    }
}

/// The comment syntax for this file kind, or `None` if it carries no notice.
fn comment_prefix(rel: &Path) -> Option<&'static str> {
    let name = rel.file_name().and_then(|n| n.to_str())?;
    if let Some((_, prefix)) = BY_NAME.iter().find(|(n, _)| *n == name) {
        return Some(prefix);
    }
    let ext = rel.extension().and_then(|e| e.to_str())?;
    BY_EXTENSION
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, p)| *p)
}

/// Whether any whole path element of `rel` names an unlicensed tree.
///
/// Whole elements, never a bare prefix: testing `starts_with(".git")` would
/// exclude `.github` along with `.git`, and quietly stop checking every CI
/// workflow in the repository.
fn is_unlicensed(rel: &Path) -> bool {
    rel.components()
        .filter_map(|c| c.as_os_str().to_str())
        .any(|element| UNLICENSED.contains(&element))
}

#[cfg(test)]
mod tests {
    use super::{is_unlicensed, missing_notice};
    use std::path::Path;

    #[test]
    fn dot_github_is_not_excluded_along_with_dot_git() {
        assert!(is_unlicensed(Path::new(".git/config")));
        assert!(!is_unlicensed(Path::new(".github/workflows/ci.yml")));
    }

    #[test]
    fn a_notice_below_the_header_does_not_count() {
        let buried = "\n\n\n\n// SPDX-License-Identifier: Apache-2.0\n";
        assert!(missing_notice(buried, "//").is_some());
    }
}
