// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The pure layers perform no I/O and read no clock.

use crate::arch::{LAYERS, SEAM};
use crate::metadata;
use crate::scan::{Findings, files};
use std::path::Path;

/// Standard library paths a pure layer may not name.
///
/// `log` and `tracing` are here beside the I/O modules deliberately. A
/// protocol codec that logs has made a policy decision on its caller's
/// behalf, and a library that narrates into somebody else's stdout is a
/// library they cannot embed. The core returns errors and spans; it does not
/// narrate.
const BANNED: &[(&str, &str)] = &[
    ("std::fs", "the pure core reads no files"),
    ("std::net", "the pure core opens no sockets"),
    ("std::process", "the pure core spawns nothing"),
    (
        "std::io::stdout",
        "the pure core writes to nobody's terminal",
    ),
    ("std::time::SystemTime", "time is injected, never read"),
    ("std::time::Instant", "time is injected, never read"),
    (
        "std::thread::sleep",
        "the core returns a deadline; the runtime waits",
    ),
    (
        "log::",
        "the core returns errors and spans; it does not narrate",
    ),
    ("tracing::", "observability goes through the telemetry seam"),
];

/// Checks that pure crates stay pure, in their sources and their manifests.
///
/// # Errors
///
/// When a pure crate performs I/O, reads a clock, or grows a dependency.
pub fn run(root: &Path) -> Result<String, String> {
    let members = metadata::workspace_members(root)?;
    let mut findings = Findings::default();

    for layer in LAYERS.iter().filter(|l| l.pure) {
        let dir = root.join("crates").join(layer.krate);

        if let Some(member) = members.iter().find(|m| m.name == layer.krate) {
            findings.checked();
            for dep in &member.direct_deps {
                if dep.name != SEAM {
                    findings.violation(format!(
                        "{} depends on {}; a pure layer's only permitted dependency is \
                         {SEAM}, and that closes its graph",
                        layer.krate, dep.name
                    ));
                }
            }
        }

        for rel in files(&dir) {
            if rel.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            findings.checked();
            let Ok(text) = std::fs::read_to_string(dir.join(&rel)) else {
                continue;
            };
            for (path, why) in BANNED {
                if names(&text, path) {
                    findings.violation(format!(
                        "{}/{} names {path}: {why}",
                        layer.krate,
                        rel.display()
                    ));
                }
            }
        }
    }

    findings.verdict("purity")
}

/// Whether the source names a banned path outside a comment.
///
/// Documentation may discuss what the layer must not do — these contracts are
/// written down precisely so a reader knows — so a line that is a comment is
/// not a use of the thing it mentions.
fn names(source: &str, path: &str) -> bool {
    source.lines().any(|line| {
        let code = line.trim_start();
        !code.starts_with("//") && !code.starts_with('*') && code.contains(path)
    })
}

#[cfg(test)]
mod tests {
    use super::names;

    #[test]
    fn a_layer_contract_may_say_what_the_layer_must_not_do() {
        assert!(!names("//! Must not: touch std::fs.\n", "std::fs"));
        assert!(names("use std::fs::File;\n", "std::fs"));
    }
}
